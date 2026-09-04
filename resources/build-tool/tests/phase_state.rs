//! Tests for kdevkit's phase-state machinery, written against the
//! requirements in `specs/feature/kdevkit-deterministic-phasing.md` rather
//! than against the implementation.
//!
//! Each test builds a real git repository in a `TempDir` and drives it with
//! ordinary git commands, the way a person would. Assertions describe what
//! someone would find in the repository afterwards — never which script ran
//! or in what order. If the mechanism were rebuilt a different way, these
//! should still pass.
//!
//! Numbered statements refer to that spec's "what must be true" list.

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

use build_tool::shared::repo_root;

// ── harness ──────────────────────────────────────────────────────

fn tools_dir() -> PathBuf {
    repo_root()
        .expect("repo root resolves under cargo test")
        .join("resources/content/skills/kdevkit/tools")
}

fn phase_bin() -> PathBuf {
    tools_dir().join("feature-loop")
}

/// Run a command in `dir`, returning (exit code, stdout+stderr).
fn run_in(dir: &Path, program: &str, args: &[&str]) -> (i32, String) {
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn {program}: {e}"));
    let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.code().unwrap_or(-1), combined)
}

fn git(dir: &Path, args: &[&str]) -> (i32, String) {
    run_in(dir, "git", args)
}

/// git, asserting success — for setup steps whose failure would make the
/// test meaningless rather than failing.
fn git_ok(dir: &Path, args: &[&str]) {
    let (code, out) = git(dir, args);
    assert_eq!(code, 0, "git {args:?} failed in setup:\n{out}");
}

fn phase(dir: &Path, args: &[&str]) -> (i32, String) {
    let bin = phase_bin();
    run_in(dir, bin.to_str().unwrap(), args)
}

fn fact<'a>(facts: &'a str, key: &str) -> &'a str {
    facts
        .lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")))
        .unwrap_or_else(|| panic!("no fact '{key}' in:\n{facts}"))
}

/// A repository with kdevkit's hooks wired up the way install does, plus a
/// feature branch and a spec naming it. Returns the worktree path.
struct Fixture {
    _tmp: TempDir,
    repo: PathBuf,
}

impl Fixture {
    /// A bare project on its default branch, no feature yet.
    fn new() -> Self {
        let tmp = TempDir::new().expect("tempdir");
        let repo = tmp.path().join("project");
        std::fs::create_dir_all(&repo).unwrap();
        git_ok(&repo, &["init", "-q", "-b", "main", "."]);
        // git refuses to commit without an identity; nothing else here is
        // configuration kdevkit needs.
        git_ok(&repo, &["config", "user.email", "dev@example.com"]);
        git_ok(&repo, &["config", "user.name", "dev"]);
        // What install does: point git at kdevkit's own hooks.
        git_ok(
            &repo,
            &[
                "config",
                "core.hooksPath",
                tools_dir().join("hooks").to_str().unwrap(),
            ],
        );
        std::fs::write(repo.join("src.txt"), "v1\n").unwrap();
        git_ok(&repo, &["add", "-A"]);
        git_ok(&repo, &["commit", "-q", "-m", "chore: init"]);
        Fixture { _tmp: tmp, repo }
    }

    fn path(&self) -> &Path {
        &self.repo
    }

    /// Start a feature: branch, spec naming it, and the plan commit.
    fn start_feature(&self, branch: &str) {
        git_ok(self.path(), &["checkout", "-q", "-b", branch]);
        self.write_spec(branch, &["- [ ] 1 · do the thing"]);
        git_ok(self.path(), &["add", "-A"]);
        git_ok(self.path(), &["commit", "-q", "-m", "plan(f): spec"]);
    }

    fn write_spec(&self, branch: &str, plan_items: &[&str]) {
        let dir = self.repo.join("specs/feature");
        std::fs::create_dir_all(&dir).unwrap();
        let body = format!(
            "# Feature\n\nBranch: {branch}\n\n## Handoff\n\n\
             - **Ready for:** dev\n- **Carry forward:** nothing\n\n\
             ## Implementation Plan\n\n{}\n",
            plan_items.join("\n")
        );
        std::fs::write(dir.join("f.md"), body).unwrap();
    }

    /// The stage recorded on the branch, as a reader would find it.
    fn recorded_stage(&self) -> String {
        let (_, out) = git(
            self.path(),
            &[
                "log",
                "--format=%(trailers:key=Kdevkit-Feature-Stage,valueonly=true,unfold=true)",
            ],
        );
        out.lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("")
            .to_string()
    }

    fn commit_count(&self) -> usize {
        let (_, out) = git(self.path(), &["log", "--oneline"]);
        out.lines().filter(|l| !l.trim().is_empty()).count()
    }

    /// Declare the project's dev gates where kdevkit declares everything
    /// else about a project: `specs/project.md`.
    fn set_checks(&self, quality: &str, tests: &str) {
        let dir = self.repo.join("specs");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("project.md"),
            format!(
                "# Project\n\n## Agent Development\n\n### kdevkit\n\
                 - `gates:`\n  - `dev:`\n    - `quality: {quality}`\n    - `tests: {tests}`\n"
            ),
        )
        .unwrap();
        // A project's own declaration is committed, like any project file —
        // and leaving it uncommitted would make the tree dirty, which
        // `verify` refuses for good reason.
        git_ok(self.path(), &["add", "-A"]);
        git_ok(
            self.path(),
            &[
                "commit",
                "-q",
                "--no-verify",
                "-m",
                "chore: declare dev gates",
            ],
        );
    }
}

// ── statement 1 · state is recoverable from the repository ───────

#[test]
fn stage_is_recoverable_from_the_repository_alone() {
    let f = Fixture::new();
    f.start_feature("feat/x");

    // Nothing was told to the checker; it reads the branch.
    let (code, out) = phase(f.path(), &["show"]);
    assert_eq!(code, 0, "show failed:\n{out}");
    assert_eq!(
        fact(&out, "stage"),
        "planning",
        "a fresh reader must recover the stage from history:\n{out}"
    );

    // And it is the same value a plain git reader would find, so the
    // checker is not the only thing that can tell.
    assert_eq!(f.recorded_stage(), "planning");
}

#[test]
fn an_unstamped_repository_reports_unrecorded_rather_than_guessing() {
    let f = Fixture::new();
    // Default branch, no feature: nothing should be claimed.
    let (_, out) = phase(f.path(), &["show"]);
    assert_eq!(fact(&out, "stage"), "unrecorded", "{out}");
    assert_eq!(fact(&out, "applies"), "no", "{out}");
}

// ── statements 2 and 3 · a contradicted claim does not land ──────

#[test]
fn a_commit_claiming_an_unreachable_stage_is_refused() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    let before = f.commit_count();

    // Claim review while no implementation commit exists.
    let (code, out) = phase(f.path(), &["advance", "--to", "review"]);
    assert_ne!(code, 0, "advance to review should be refused:\n{out}");
    assert!(
        out.contains("not a forward move") || out.contains("refused:"),
        "the refusal must say why:\n{out}"
    );

    // Nothing was recorded, so no commit could carry the claim.
    assert_eq!(f.commit_count(), before, "no commit should have been made");
    assert_eq!(f.recorded_stage(), "planning", "stage must be unchanged");
}

#[test]
fn a_commit_whose_recorded_intent_no_longer_holds_is_refused() {
    let f = Fixture::new();
    f.start_feature("feat/x");

    // Legitimately record a move to dev, then break the precondition by
    // removing the spec that names the branch.
    let (code, out) = phase(f.path(), &["advance", "--to", "dev"]);
    assert_eq!(code, 0, "advance to dev should be permitted:\n{out}");
    std::fs::remove_file(f.path().join("specs/feature/f.md")).unwrap();

    let before = f.commit_count();
    std::fs::write(f.path().join("src.txt"), "v2\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    let (code, out) = git(f.path(), &["commit", "-m", "feat: work"]);
    assert_ne!(code, 0, "the commit should be refused:\n{out}");
    assert_eq!(
        f.commit_count(),
        before,
        "the refused commit must not exist:\n{out}"
    );
}

#[test]
fn the_refusal_survives_no_verify() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    phase(f.path(), &["advance", "--to", "dev"]);
    std::fs::remove_file(f.path().join("specs/feature/f.md")).unwrap();

    let before = f.commit_count();
    std::fs::write(f.path().join("src.txt"), "v2\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    // The whole point: --no-verify must not buy a way past the gate.
    let (code, out) = git(f.path(), &["commit", "--no-verify", "-m", "feat: work"]);
    assert_ne!(code, 0, "--no-verify must not bypass the refusal:\n{out}");
    assert_eq!(
        f.commit_count(),
        before,
        "the commit must not exist:\n{out}"
    );
}

// ── the stamp is derived, not claimed ────────────────────────────

#[test]
fn every_feature_commit_is_stamped_without_being_asked() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    // The plan commit was stamped even though nothing requested it.
    assert_eq!(f.recorded_stage(), "planning");

    phase(f.path(), &["advance", "--to", "dev"]);
    std::fs::write(f.path().join("src.txt"), "v2\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "feat: work"]);
    assert_eq!(f.recorded_stage(), "dev");
}

#[test]
fn an_amended_commit_cannot_launder_away_its_stamp() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    assert_eq!(f.recorded_stage(), "planning");

    // Replace the entire message, which destroys trailers.
    git_ok(
        f.path(),
        &["commit", "-q", "--amend", "-m", "plan(f): reworded"],
    );
    assert_eq!(
        f.recorded_stage(),
        "planning",
        "an amend must be re-stamped, not left bare"
    );
}

// ── statements 4, 5, 6 · going back ──────────────────────────────

#[test]
fn going_back_requires_naming_the_fault_the_problem_and_the_fix() {
    let f = Fixture::new();
    f.start_feature("feat/x");

    // Each missing field must be refused on its own.
    let (code, out) = phase(f.path(), &["return", "--to", "planning"]);
    assert_ne!(code, 0, "a bare return must be refused:\n{out}");
    assert!(out.contains("fault-entered"), "{out}");

    let (code, out) = phase(
        f.path(),
        &[
            "return",
            "--to",
            "planning",
            "--fault-entered",
            "requirements",
        ],
    );
    assert_ne!(code, 0, "a return without an issue must be refused:\n{out}");
    assert!(out.contains("issue"), "{out}");
}

#[test]
fn a_recorded_return_names_its_reasons_and_is_counted() {
    let f = Fixture::new();
    f.start_feature("feat/x");

    let (code, out) = phase(
        f.path(),
        &[
            "return",
            "--to",
            "planning",
            "--fault-entered",
            "requirements",
            "--issue",
            "warning suppression was never specified",
            "--expected-fix",
            "amend R2 and extend the tests",
            "--acceptance",
            "warnings are suppressed with the flag",
        ],
    );
    assert_eq!(code, 0, "a complete return should be accepted:\n{out}");

    std::fs::write(f.path().join("src.txt"), "v3\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "plan(f): amend R2"]);

    let (_, facts) = phase(f.path(), &["facts"]);
    assert_eq!(fact(&facts, "return_count"), "1", "{facts}");
    assert_eq!(fact(&facts, "stage_recorded"), "planning", "{facts}");
}

#[test]
fn an_unresolved_return_blocks_a_move_that_would_otherwise_be_legal() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    phase(
        f.path(),
        &[
            "return",
            "--to",
            "planning",
            "--fault-entered",
            "requirements",
            "--issue",
            "i",
            "--expected-fix",
            "fx",
            "--acceptance",
            "a",
        ],
    );
    std::fs::write(f.path().join("src.txt"), "v3\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(
        f.path(),
        &["commit", "-q", "-m", "plan(f): record the return"],
    );

    // planning -> dev is a legal edge and every other precondition holds,
    // so only the open return can be what refuses this.
    let (code, out) = phase(f.path(), &["check", "--to", "dev"]);
    assert_ne!(
        code, 0,
        "an undischarged return must block a legal forward move:\n{out}"
    );
    assert!(out.contains("return to"), "must say why:\n{out}");

    // Doing the work discharges it.
    std::fs::write(f.path().join("src.txt"), "v4\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(
        f.path(),
        &["commit", "-q", "-m", "plan(f): amend the requirement"],
    );
    let (code, out) = phase(f.path(), &["check", "--to", "dev"]);
    assert_eq!(
        code, 0,
        "the fix commit should discharge the return:\n{out}"
    );
}

#[test]
fn an_illegal_edge_is_refused_even_when_every_precondition_holds() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    // Give closure everything it asks for: a published branch.
    let remote = f.path().parent().unwrap().join("remote.git");
    git_ok(
        f.path(),
        &["init", "-q", "--bare", remote.to_str().unwrap()],
    );
    git_ok(
        f.path(),
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git_ok(f.path(), &["push", "-q", "--no-verify", "origin", "feat/x"]);

    // planning -> closure skips dev and review. Preconditions for closure
    // are satisfied, so the closed table is the only thing that can refuse.
    let (code, out) = phase(f.path(), &["check", "--to", "closure"]);
    assert_ne!(code, 0, "skipping stages must be refused:\n{out}");
    assert!(
        out.contains("not a forward move"),
        "must name the reason as an illegal move:\n{out}"
    );
}

#[test]
fn advance_cannot_be_used_to_go_backwards() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    phase(f.path(), &["advance", "--to", "dev"]);
    std::fs::write(f.path().join("src.txt"), "v2\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "feat: work"]);
    assert_eq!(f.recorded_stage(), "dev");

    // Going back is a `return`, which demands reasons. `advance` must not
    // offer a way to do it quietly.
    let (code, out) = phase(f.path(), &["advance", "--to", "planning"]);
    assert_ne!(code, 0, "advance must not move backwards:\n{out}");
    assert!(out.contains("use 'return'"), "must point at return:\n{out}");
    assert_eq!(f.recorded_stage(), "dev", "the stage must be unchanged");
}

// ── statement 9 · undetermined means no ──────────────────────────

#[test]
fn an_undeterminable_state_refuses_rather_than_guessing() {
    let f = Fixture::new();
    git_ok(f.path(), &["checkout", "-q", "-b", "feat/orphan"]);
    // A branch with a stage but no spec: the checker cannot evaluate
    // preconditions and must say so rather than permit.
    let (code, out) = phase(f.path(), &["check", "--to", "dev"]);
    assert_ne!(code, 0, "must not permit when it cannot tell:\n{out}");
}

// ── statement 12 · works without worktrees, inert elsewhere ──────

#[test]
fn commits_on_the_default_branch_are_untouched() {
    let f = Fixture::new();
    // A feature exists, but we are committing on main.
    f.start_feature("feat/x");
    git_ok(f.path(), &["checkout", "-q", "main"]);

    std::fs::write(f.path().join("other.txt"), "x\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    let (code, out) = git(f.path(), &["commit", "-m", "chore: unrelated"]);
    assert_eq!(code, 0, "a commit on main must not be refused:\n{out}");

    let (_, msg) = git(f.path(), &["log", "-1", "--format=%B"]);
    assert!(
        !msg.contains("Kdevkit-Feature-Stage"),
        "main must not be stamped:\n{msg}"
    );
}

#[test]
fn commits_on_an_unrelated_branch_are_untouched() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    git_ok(f.path(), &["checkout", "-q", "main"]);
    git_ok(f.path(), &["checkout", "-q", "-b", "chore/cleanup"]);

    std::fs::write(f.path().join("other.txt"), "y\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    let (code, out) = git(f.path(), &["commit", "-m", "chore: tidy"]);
    assert_eq!(code, 0, "an unrelated branch must not be refused:\n{out}");

    let (_, msg) = git(f.path(), &["log", "-1", "--format=%B"]);
    assert!(
        !msg.contains("Kdevkit-Feature-Stage"),
        "an unrelated branch must not be stamped:\n{msg}"
    );
}

// ── statement 16 · outer-loop work carries no feature stage ──────

#[test]
fn initiative_work_carries_no_feature_stage() {
    let f = Fixture::new();
    git_ok(f.path(), &["checkout", "-q", "-b", "plan/initiative-x"]);
    // Initiative-level work: a spec, but under specs/initiative, and no
    // feature spec naming this branch.
    let dir = f.path().join("specs/initiative");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("x.md"),
        "# Initiative\n\nBranch: plan/initiative-x\n",
    )
    .unwrap();
    git_ok(f.path(), &["add", "-A"]);
    let (code, out) = git(f.path(), &["commit", "-m", "plan(x): initiative"]);
    assert_eq!(code, 0, "initiative work must not be refused:\n{out}");

    let (_, msg) = git(f.path(), &["log", "-1", "--format=%B"]);
    assert!(
        !msg.contains("Kdevkit-Feature-Stage"),
        "initiative work must carry no feature stage:\n{msg}"
    );
}

// ── statements 13, 14, 15 · the dev loop ─────────────────────────

#[test]
fn dev_loop_iterations_do_not_change_the_stage_or_count_as_returns() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    phase(f.path(), &["advance", "--to", "dev"]);

    // Four turns of the inner loop: work, a test fix, a review fix, more work.
    for (i, msg) in [
        "feat: first cut",
        "fix: make the tests pass",
        "refactor: address review",
        "test: cover the edge case",
    ]
    .iter()
    .enumerate()
    {
        std::fs::write(f.path().join("src.txt"), format!("v{i}\n")).unwrap();
        git_ok(f.path(), &["add", "-A"]);
        git_ok(f.path(), &["commit", "-q", "-m", msg]);
        assert_eq!(
            f.recorded_stage(),
            "dev",
            "the stage must stay dev through inner-loop turn {i}"
        );
    }

    let (_, facts) = phase(f.path(), &["facts"]);
    assert_eq!(
        fact(&facts, "return_count"),
        "0",
        "inner-loop churn must not read as thrash:\n{facts}"
    );
}

#[test]
fn leaving_dev_requires_the_checks_to_have_been_observed_passing() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    phase(f.path(), &["advance", "--to", "dev"]);
    f.write_spec("feat/x", &["- [x] 1 · do the thing"]);
    std::fs::write(f.path().join("src.txt"), "done\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "feat: implement"]);

    // Plan ticked and work committed, but nothing verified yet.
    let (code, out) = phase(f.path(), &["check", "--to", "review"]);
    assert_ne!(code, 0, "review must be refused unverified:\n{out}");
    assert!(
        out.contains("gates have not been observed passing"),
        "must say why:\n{out}"
    );

    // Verify with a passing check, then it is allowed.
    f.set_checks("true", "true");
    let (code, out) = phase(f.path(), &["verify"]);
    assert_eq!(code, 0, "verify should pass with a passing check:\n{out}");
    let (code, out) = phase(f.path(), &["check", "--to", "review"]);
    assert_eq!(code, 0, "review should now be permitted:\n{out}");
}

#[test]
fn a_failing_check_refuses_and_records_nothing() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    f.set_checks("true", "false");
    let (code, out) = phase(f.path(), &["verify"]);
    assert_ne!(code, 0, "a failing test command must refuse:\n{out}");
    assert!(
        out.contains("tests failed"),
        "must name what failed:\n{out}"
    );

    let (_, facts) = phase(f.path(), &["facts"]);
    assert_eq!(
        fact(&facts, "checks_verified"),
        "no",
        "a failure must not record a pass:\n{facts}"
    );
}

#[test]
fn verification_is_invalidated_by_editing_the_files_it_covered() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    phase(f.path(), &["advance", "--to", "dev"]);
    f.write_spec("feat/x", &["- [x] 1 · do the thing"]);
    std::fs::write(f.path().join("src.txt"), "done\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "feat: implement"]);
    f.set_checks("true", "true");
    phase(f.path(), &["verify"]);
    assert_eq!(
        fact(&phase(f.path(), &["facts"]).1, "checks_verified"),
        "yes"
    );

    // Change a file and commit: the old result described a different tree.
    std::fs::write(f.path().join("src.txt"), "sneaky\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "fix: sneak a change in"]);

    let (_, facts) = phase(f.path(), &["facts"]);
    assert_eq!(
        fact(&facts, "checks_verified"),
        "no",
        "verification must not survive an edit:\n{facts}"
    );
    let (code, out) = phase(f.path(), &["check", "--to", "review"]);
    assert_ne!(
        code, 0,
        "a stale verification must not permit review:\n{out}"
    );
}

#[test]
fn verify_refuses_when_tracked_files_are_modified() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    f.set_checks("true", "true");
    // A modified tracked file changes the tree, so a result recorded against
    // it would describe no commit.
    std::fs::write(f.path().join("src.txt"), "uncommitted\n").unwrap();

    let (code, out) = phase(f.path(), &["verify"]);
    assert_ne!(code, 0, "modified tracked files must refuse:\n{out}");
    assert!(
        out.contains("tracked files are modified"),
        "must say why:\n{out}"
    );
    assert!(
        out.contains("to resolve:"),
        "must name the way forward:\n{out}"
    );
}

#[test]
fn verify_ignores_untracked_files_because_they_are_not_in_the_tree() {
    // A real agent run hit this: an unrelated untracked file made `verify`
    // refuse, so the agent had to record an exception for something
    // mundane. Untracked files do not change the tree hash, so they cannot
    // invalidate a result recorded against it — and forcing exceptions for
    // trivia would make exceptions routine and therefore meaningless.
    let f = Fixture::new();
    f.start_feature("feat/x");
    f.set_checks("true", "true");
    std::fs::write(f.path().join("NOTES-not-mine.md"), "unrelated\n").unwrap();

    let (code, out) = phase(f.path(), &["verify"]);
    assert_eq!(code, 0, "an untracked file must not block verify:\n{out}");
    let (_, facts) = phase(f.path(), &["facts"]);
    assert_eq!(fact(&facts, "checks_verified"), "yes", "{facts}");
}

// ── the push gate ────────────────────────────────────────────────

#[test]
fn a_branch_with_failing_checks_cannot_be_published() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    let remote = f.path().parent().unwrap().join("remote.git");
    git_ok(
        f.path(),
        &["init", "-q", "--bare", remote.to_str().unwrap()],
    );
    git_ok(
        f.path(),
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    f.set_checks("true", "false");

    let (code, out) = git(f.path(), &["push", "origin", "feat/x"]);
    assert_ne!(code, 0, "the push must be refused:\n{out}");

    // Assert against the REMOTE, not the local branch — the local branch
    // would pass this test even if the push had succeeded.
    let (_, refs) = git(f.path(), &["ls-remote", "origin"]);
    assert!(
        !refs.contains("feat/x"),
        "the failing commit must not have reached the remote:\n{refs}"
    );
}

#[test]
fn a_branch_with_passing_checks_can_be_published() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    let remote = f.path().parent().unwrap().join("remote.git");
    git_ok(
        f.path(),
        &["init", "-q", "--bare", remote.to_str().unwrap()],
    );
    git_ok(
        f.path(),
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    f.set_checks("true", "true");

    let (code, out) = git(f.path(), &["push", "origin", "feat/x"]);
    assert_eq!(code, 0, "a green branch must publish:\n{out}");
    let (_, refs) = git(f.path(), &["ls-remote", "origin"]);
    assert!(
        refs.contains("feat/x"),
        "the branch should be there:\n{refs}"
    );
}

// ── closure precondition ─────────────────────────────────────────

#[test]
fn closure_requires_the_branch_to_have_been_published() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    f.write_spec("feat/x", &["- [x] 1 · do the thing"]);
    std::fs::write(f.path().join("src.txt"), "done\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    phase(f.path(), &["advance", "--to", "dev"]);
    git_ok(f.path(), &["commit", "-q", "-m", "feat: implement"]);
    f.set_checks("true", "true");
    phase(f.path(), &["verify"]);
    phase(f.path(), &["advance", "--to", "review"]);
    std::fs::write(f.path().join("src.txt"), "done2\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "test: cover it"]);
    assert_eq!(f.recorded_stage(), "review");

    // No remote at all: review cannot have happened.
    let (code, out) = phase(f.path(), &["check", "--to", "closure"]);
    assert_ne!(code, 0, "closure must be refused unpublished:\n{out}");
    assert!(out.contains("not on the remote"), "must say why:\n{out}");
}

// ── facts always accompany a verdict ─────────────────────────────

#[test]
fn a_verdict_never_hides_its_inputs() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    for args in [
        vec!["check", "--to", "dev"],
        vec!["check", "--to", "review"],
    ] {
        let (_, out) = phase(f.path(), &args);
        for key in ["branch=", "stage_recorded=", "handoff_blocks="] {
            assert!(
                out.contains(key),
                "{args:?} must print facts alongside its verdict; missing {key}:\n{out}"
            );
        }
    }
}

// ── the record cannot fall behind reality ─────────────────────────

#[test]
fn implementation_work_records_dev_even_if_the_agent_never_advanced() {
    // Found by a paid agent run: all three agents did the work and committed
    // without calling `advance`, and the stamp carried `planning` forward, so
    // the record silently lied. The stage must follow the evidence.
    let f = Fixture::new();
    f.start_feature("feat/x");
    assert_eq!(f.recorded_stage(), "planning");

    // No advance. Just implementation work, as an agent under load does.
    std::fs::write(f.path().join("src.txt"), "implemented\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(
        f.path(),
        &["commit", "-q", "-m", "feat: implement the thing"],
    );

    assert_eq!(
        f.recorded_stage(),
        "dev",
        "a commit that is implementation work is dev, said or not"
    );
}

#[test]
fn the_record_never_slides_backwards_on_its_own() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    phase(f.path(), &["advance", "--to", "dev"]);
    // Distinct from the fixture's seeded content, or there is nothing to commit.
    std::fs::write(f.path().join("src.txt"), "implemented\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "feat: work"]);
    f.write_spec("feat/x", &["- [x] 1 · done"]);
    f.set_checks("true", "true");
    phase(f.path(), &["verify"]);
    phase(f.path(), &["advance", "--to", "review"]);
    std::fs::write(f.path().join("src.txt"), "v2\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "test: cover it"]);
    assert_eq!(f.recorded_stage(), "review");

    // A further implementation commit implies dev, but the record is already
    // at review. Going back is a `return`, never a side effect of a commit.
    std::fs::write(f.path().join("src.txt"), "v3\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(
        f.path(),
        &["commit", "-q", "-m", "fix: address a review note"],
    );
    assert_eq!(
        f.recorded_stage(),
        "review",
        "the record must not slide back to dev without a return"
    );
}

#[test]
fn a_return_rewinds_what_counts_as_evidence() {
    // Without this, the commit that records a return is immediately pulled
    // forward again by the implementation commits that preceded it — the
    // return would be undone by the next commit. Caught first by the
    // lifecycle suite; pinned here so it fails at both layers.
    let f = Fixture::new();
    f.start_feature("feat/x");
    phase(f.path(), &["advance", "--to", "dev"]);
    std::fs::write(f.path().join("src.txt"), "implemented\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "feat: implement"]);
    assert_eq!(f.recorded_stage(), "dev");

    phase(
        f.path(),
        &[
            "return",
            "--to",
            "planning",
            "--fault-entered",
            "requirements",
            "--issue",
            "i",
            "--expected-fix",
            "fx",
            "--acceptance",
            "a",
        ],
    );
    std::fs::write(f.path().join("src.txt"), "returned\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "plan: record the return"]);
    assert_eq!(
        f.recorded_stage(),
        "planning",
        "the return must take effect"
    );

    // The next commit is planning work. The earlier feat( commits must no
    // longer imply dev, or the return silently evaporates.
    std::fs::write(f.path().join("src.txt"), "amended\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(
        f.path(),
        &["commit", "-q", "-m", "plan: amend the requirement"],
    );
    assert_eq!(
        f.recorded_stage(),
        "planning",
        "work done before the return must not pull the record forward again"
    );
}

#[test]
fn a_close_commit_records_closure_without_anyone_advancing() {
    // Found by an agent that did the closure work correctly but never called
    // `advance`, leaving the record at dev. kdevkit's own convention gives
    // closure a signature -- a `close(...)` commit -- so it is derivable.
    let f = Fixture::new();
    f.start_feature("feat/x");
    std::fs::write(f.path().join("src.txt"), "implemented\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "feat: implement"]);
    assert_eq!(f.recorded_stage(), "dev");

    std::fs::write(f.path().join("src.txt"), "closing\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "close(x): archive the spec"]);
    assert_eq!(
        f.recorded_stage(),
        "closure",
        "a close() commit is closure, said or not"
    );
}

#[test]
fn review_is_never_inferred_because_nothing_observable_marks_it() {
    // Review alone has no signature: nothing a commit contains distinguishes
    // "a human has reviewed this". Inferring it would let the machinery claim
    // a review happened that never did.
    let f = Fixture::new();
    f.start_feature("feat/x");
    f.write_spec("feat/x", &["- [x] 1 · done"]);
    f.set_checks("true", "true");
    std::fs::write(f.path().join("src.txt"), "done\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(
        f.path(),
        &["commit", "-q", "-m", "feat: everything is finished"],
    );
    phase(f.path(), &["verify"]);
    // Every precondition for review now holds, and still:
    assert_eq!(
        f.recorded_stage(),
        "dev",
        "review must be recorded deliberately, never inferred"
    );
}

// ── the tooling owns the map ──────────────────────────────────────

#[test]
fn the_onward_stage_comes_from_the_tooling_not_the_caller() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    // A caller that knows only "I am done" gets told where that leads.
    let (code, out) = phase(f.path(), &["next"]);
    assert_eq!(code, 0, "next failed:\n{out}");
    assert_eq!(out.trim(), "dev", "planning must lead to dev:\n{out}");
}

#[test]
fn advance_next_moves_on_without_the_caller_naming_a_destination() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    let (code, out) = phase(f.path(), &["advance", "--next", "--ack", "human"]);
    assert_eq!(code, 0, "advance --next failed:\n{out}");
    std::fs::write(f.path().join("src.txt"), "v2\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "feat: work"]);
    assert_eq!(f.recorded_stage(), "dev");
}

#[test]
fn advance_next_still_refuses_when_the_exit_condition_does_not_hold() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    phase(f.path(), &["advance", "--next"]);
    std::fs::write(f.path().join("src.txt"), "v2\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "feat: work"]);
    assert_eq!(f.recorded_stage(), "dev");

    // dev -> review is the legal next move, but the plan is unticked and
    // nothing is verified. Asking for "next" must not bypass that.
    let (code, out) = phase(f.path(), &["advance", "--next"]);
    assert_ne!(
        code, 0,
        "--next must be gated exactly as a named move is:\n{out}"
    );
    assert!(out.contains("unticked"), "must say why:\n{out}");
}

#[test]
fn a_closed_feature_has_no_onward_stage() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    // Walk to closed the short way, then ask.
    for stage in ["dev", "review", "closure", "closed"] {
        git_ok(
            f.path(),
            &[
                "commit",
                "-q",
                "--allow-empty",
                "--no-verify",
                "-m",
                &format!("chore: at {stage}\n\nKdevkit-Feature-Stage: {stage}"),
            ],
        );
    }
    assert_eq!(f.recorded_stage(), "closed");
    let (code, out) = phase(f.path(), &["next"]);
    assert_eq!(code, 3, "a closed feature has nowhere to go:\n{out}");
    assert!(out.contains("closed"), "must say why:\n{out}");
}

// ── the return record survives on the branch ──────────────────────

/// Trailer values for a key, newest first.
fn trailers(dir: &Path, key: &str) -> Vec<String> {
    let (_, out) = git(
        dir,
        &[
            "log",
            &format!("--format=%(trailers:key={key},valueonly=true,unfold=true)"),
        ],
    );
    out.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

#[test]
fn a_return_records_all_four_of_its_reasons_on_the_branch() {
    // The four fields ARE the record a later builder reads. An earlier
    // version validated them and threw them away, which removed the reason
    // this feature exists.
    let f = Fixture::new();
    f.start_feature("feat/x");
    phase(
        f.path(),
        &[
            "return",
            "--to",
            "planning",
            "--fault-entered",
            "requirements",
            "--issue",
            "warning suppression was never specified",
            "--expected-fix",
            "amend R2 and extend the tests",
            "--acceptance",
            "warnings are suppressed with the flag",
        ],
    );
    std::fs::write(f.path().join("src.txt"), "recorded\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "plan: record the return"]);

    // Read them back the way a later builder would: from the branch.
    for (key, expect) in [
        ("Kdevkit-Feature-Return", "planning"),
        ("Kdevkit-Feature-Return-Fault", "requirements"),
        (
            "Kdevkit-Feature-Return-Issue",
            "warning suppression was never specified",
        ),
        (
            "Kdevkit-Feature-Return-Fix",
            "amend R2 and extend the tests",
        ),
        (
            "Kdevkit-Feature-Return-Acceptance",
            "warnings are suppressed with the flag",
        ),
    ] {
        let got = trailers(f.path(), key);
        assert_eq!(
            got.first().map(String::as_str),
            Some(expect),
            "{key} must be readable from the branch; found {got:?}"
        );
    }
}

// ── proceeding past a gate, on the record ─────────────────────────

#[test]
fn an_exception_needs_what_and_why() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    for args in [
        vec!["except", "--to", "dev"],
        vec!["except", "--to", "dev", "--skipping", "plan items"],
    ] {
        let (code, out) = phase(f.path(), &args);
        assert_ne!(code, 0, "{args:?} must be refused:\n{out}");
    }
}

#[test]
fn an_exception_gets_past_a_gate_and_is_recorded_and_counted() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    phase(f.path(), &["advance", "--to", "dev"]);
    std::fs::write(f.path().join("src.txt"), "work\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "feat: work"]);

    // The gate genuinely refuses: plan unticked, nothing verified.
    let (code, _) = phase(f.path(), &["advance", "--to", "review"]);
    assert_ne!(code, 0, "the gate should refuse first");

    let (code, out) = phase(
        f.path(),
        &[
            "except",
            "--to",
            "review",
            "--skipping",
            "the dev gates",
            "--why",
            "release deadline; follow-up filed as #12",
        ],
    );
    assert_eq!(code, 0, "an explained exception must be allowed:\n{out}");
    std::fs::write(f.path().join("src.txt"), "shipping\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "test: ship it"]);

    assert_eq!(f.recorded_stage(), "review", "the move must have happened");
    assert_eq!(
        trailers(f.path(), "Kdevkit-Feature-Exception")
            .first()
            .map(String::as_str),
        Some("the dev gates"),
        "what was skipped must be on the branch"
    );
    assert_eq!(
        trailers(f.path(), "Kdevkit-Feature-Exception-Why")
            .first()
            .map(String::as_str),
        Some("release deadline; follow-up filed as #12"),
    );
    let out = phase(f.path(), &["show"]).1;
    assert!(
        out.contains("exceptions=1"),
        "an exception must be visible at a glance:\n{out}"
    );
}

#[test]
fn an_exception_cannot_reorder_the_stages() {
    // It waives a precondition, not the sequence.
    let f = Fixture::new();
    f.start_feature("feat/x");
    let (code, out) = phase(
        f.path(),
        &[
            "except",
            "--to",
            "closure",
            "--skipping",
            "everything",
            "--why",
            "because",
        ],
    );
    assert_ne!(code, 0, "skipping stages must still be refused:\n{out}");
    assert!(out.contains("not reorder"), "must say why:\n{out}");
}

// ── counts are surfaced as decision input ─────────────────────────

#[test]
fn repeated_attempts_in_one_stage_are_surfaced_as_a_hint() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    phase(f.path(), &["advance", "--to", "dev"]);
    for i in 0..3 {
        std::fs::write(f.path().join("src.txt"), format!("try{i}\n")).unwrap();
        git_ok(f.path(), &["add", "-A"]);
        git_ok(
            f.path(),
            &["commit", "-q", "-m", &format!("fix: attempt {i}")],
        );
    }
    let out = phase(f.path(), &["show"]).1;
    assert!(
        out.contains("attempts_in_stage=3"),
        "attempts must be counted:\n{out}"
    );
    assert!(
        out.contains("fault may be in an earlier layer"),
        "repeated failure must be surfaced as decision input:\n{out}"
    );
}

// ── the branch line is matched in the shapes specs really use ─────

#[test]
fn the_spec_is_found_in_every_branch_line_format_this_project_uses() {
    // A literal match on one shape made the mechanism silently inert on
    // every real spec in this repository.
    for form in [
        "Branch: feat/x",
        "- Branch: `feat/x`",
        "**Branch:** feat/x",
        "- **Branch:** `feat/x`",
    ] {
        let f = Fixture::new();
        git_ok(f.path(), &["checkout", "-q", "-b", "feat/x"]);
        let dir = f.path().join("specs/feature");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("f.md"),
            format!("# F\n\n{form}\n\n## Handoff\n\n- **Ready for:** dev\n"),
        )
        .unwrap();
        let (_, facts) = phase(f.path(), &["facts"]);
        assert_eq!(
            fact(&facts, "applies"),
            "yes",
            "format {form:?} must be recognised:\n{facts}"
        );
    }
}

#[test]
fn a_branch_merely_mentioned_in_prose_does_not_qualify_a_spec() {
    let f = Fixture::new();
    git_ok(f.path(), &["checkout", "-q", "-b", "feat/x"]);
    let dir = f.path().join("specs/feature");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("f.md"),
        "# F\n\nSee also feat/x for context.\n\n## Handoff\n",
    )
    .unwrap();
    let (_, facts) = phase(f.path(), &["facts"]);
    assert_eq!(fact(&facts, "applies"), "no", "{facts}");
}

// ── every refusal names the way forward ──────────────────────────

#[test]
fn every_refusal_says_what_would_resolve_it() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    phase(f.path(), &["advance", "--to", "dev"]);
    std::fs::write(f.path().join("src.txt"), "work\n").unwrap();
    git_ok(f.path(), &["add", "-A"]);
    git_ok(f.path(), &["commit", "-q", "-m", "feat: work"]);

    for args in [
        vec!["check", "--to", "review"],
        vec!["check", "--to", "closure"],
    ] {
        let (code, out) = phase(f.path(), &args);
        assert_ne!(code, 0, "{args:?} should refuse here:\n{out}");
        assert!(
            out.contains("to resolve:"),
            "{args:?} refused without naming a way forward:\n{out}"
        );
    }
}

// ── install wiring ───────────────────────────────────────────────

#[test]
fn install_wires_the_hooks_and_uninstall_removes_them() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("p");
    std::fs::create_dir_all(&repo).unwrap();
    git_ok(&repo, &["init", "-q", "-b", "main", "."]);
    git_ok(&repo, &["config", "user.email", "d@e.f"]);
    git_ok(&repo, &["config", "user.name", "d"]);
    assert_eq!(
        git(&repo, &["config", "--get", "core.hooksPath"]).0,
        1,
        "a fresh repo should have no hooks path"
    );

    let (code, out) = phase(&repo, &["install"]);
    assert_eq!(code, 0, "install failed:\n{out}");
    let (_, path) = git(&repo, &["config", "--get", "core.hooksPath"]);
    assert!(
        path.trim().ends_with("kdevkit/tools/hooks"),
        "hooks path should point at kdevkit:\n{path}"
    );

    let (code, out) = phase(&repo, &["uninstall"]);
    assert_eq!(code, 0, "uninstall failed:\n{out}");
    assert_eq!(
        git(&repo, &["config", "--get", "core.hooksPath"]).0,
        1,
        "uninstall must leave no hooks path behind"
    );
}

#[test]
fn install_never_chains_a_kdevkit_hook_to_itself() {
    // A real agent run hit this: another checkout's copy of the same hook
    // has a different path, passed a path-equality guard, and got chained —
    // so prepare-commit-msg called itself until the shell died at depth
    // 1000, which bricks committing. Identify ours by content, not path.
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("p");
    let twin = tmp.path().join("another-checkout-of-kdevkit/hooks");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&twin).unwrap();
    // A copy of our own hooks at a different path.
    for h in ["prepare-commit-msg", "pre-push"] {
        std::fs::copy(tools_dir().join("hooks").join(h), twin.join(h)).unwrap();
    }

    git_ok(&repo, &["init", "-q", "-b", "main", "."]);
    git_ok(&repo, &["config", "user.email", "d@e.f"]);
    git_ok(&repo, &["config", "user.name", "d"]);
    git_ok(&repo, &["config", "core.hooksPath", twin.to_str().unwrap()]);

    let (code, out) = phase(&repo, &["install"]);
    assert_eq!(code, 0, "install failed:\n{out}");
    assert!(
        !out.contains("chained existing"),
        "install must not chain a copy of itself:\n{out}"
    );
    for key in ["kdevkit.chain.prepareCommitMsg", "kdevkit.chain.prePush"] {
        assert_eq!(
            git(&repo, &["config", "--get", key]).0,
            1,
            "{key} must not be set to a copy of our own hook"
        );
    }

    // And committing must actually work rather than recursing to death.
    std::fs::create_dir_all(repo.join("specs/feature")).unwrap();
    std::fs::write(
        repo.join("specs/feature/f.md"),
        "# F\n\n- Branch: `main`\n\n## Handoff\n",
    )
    .unwrap();
    git_ok(&repo, &["add", "-A"]);
    let (code, out) = git(&repo, &["commit", "-q", "-m", "chore: init"]);
    assert_eq!(code, 0, "committing must not recurse:\n{out}");
}

#[test]
fn install_does_not_silently_displace_an_existing_hook() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("p");
    let mine = tmp.path().join("myhooks");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&mine).unwrap();
    // Someone's own hook, which must survive kdevkit taking the slot.
    let marker = tmp.path().join("their-hook-ran");
    let hook = mine.join("prepare-commit-msg");
    std::fs::write(
        &hook,
        format!("#!/bin/sh\necho ran >> {}\n", marker.display()),
    )
    .unwrap();
    let mut perms = std::fs::metadata(&hook).unwrap().permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o755);
    }
    std::fs::set_permissions(&hook, perms).unwrap();

    git_ok(&repo, &["init", "-q", "-b", "main", "."]);
    git_ok(&repo, &["config", "user.email", "d@e.f"]);
    git_ok(&repo, &["config", "user.name", "d"]);
    git_ok(&repo, &["config", "core.hooksPath", mine.to_str().unwrap()]);

    let (code, out) = phase(&repo, &["install"]);
    assert_eq!(code, 0, "install failed:\n{out}");
    assert!(
        out.contains("chained existing prepare-commit-msg"),
        "install must report what it chained:\n{out}"
    );

    // A commit on a non-feature branch: kdevkit does nothing, but the
    // displaced hook must still run.
    std::fs::write(repo.join("f.txt"), "x\n").unwrap();
    git_ok(&repo, &["add", "-A"]);
    git_ok(&repo, &["commit", "-q", "-m", "chore: init"]);
    assert!(
        marker.exists(),
        "the pre-existing hook must still run after kdevkit takes the slot"
    );
}

// ── the handoff block keeps no machine field ─────────────────────

#[test]
fn exactly_one_handoff_section_is_required() {
    let f = Fixture::new();
    f.start_feature("feat/x");
    // Two handoff sections is an ambiguous spec, not a passable one.
    let spec = f.path().join("specs/feature/f.md");
    let body = std::fs::read_to_string(&spec).unwrap() + "\n## Handoff\n\n- **Ready for:** dev\n";
    std::fs::write(&spec, body).unwrap();

    let (code, out) = phase(f.path(), &["check", "--to", "dev"]);
    assert_ne!(code, 0, "two handoff sections must be refused:\n{out}");
    assert!(out.contains("exactly one"), "must say why:\n{out}");
}
