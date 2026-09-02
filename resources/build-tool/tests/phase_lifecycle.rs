//! Whole-lifecycle tests: a feature driven from nothing to closed, the way
//! a session would drive it.
//!
//! `phase_state.rs` tests each guarantee in isolation. This file tests them
//! *together*, because the question that matters is not "does the checker
//! refuse an unticked plan" but "does a feature actually get through all
//! five stages, and does a wrong turn actually get caught". Nothing here
//! reaches into the implementation: every step is a git command or a
//! `phase` call, and every assertion is something a person could see by
//! reading the repository.
//!
//! What these do NOT cover: whether a real coding agent chooses to run
//! these steps. That is what the `.smoke` fixtures are for, and they cost
//! money to run. These tests answer the other half — that if the steps are
//! taken, the machinery holds, and if a step is skipped, something notices.

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

fn run_in(dir: &Path, program: &str, args: &[&str]) -> (i32, String) {
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn {program}: {e}"));
    let mut s = String::from_utf8_lossy(&out.stdout).to_string();
    s.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.code().unwrap_or(-1), s)
}

fn git(dir: &Path, args: &[&str]) -> (i32, String) {
    run_in(dir, "git", args)
}

fn git_ok(dir: &Path, args: &[&str]) {
    let (code, out) = git(dir, args);
    assert_eq!(code, 0, "git {args:?} failed:\n{out}");
}

fn phase(dir: &Path, args: &[&str]) -> (i32, String) {
    run_in(
        dir,
        tools_dir().join("feature-loop").to_str().unwrap(),
        args,
    )
}

fn phase_ok(dir: &Path, args: &[&str]) -> String {
    let (code, out) = phase(dir, args);
    assert_eq!(code, 0, "phase {args:?} should have succeeded:\n{out}");
    out
}

/// A project with a remote, as any real one has.
struct Project {
    _tmp: TempDir,
    repo: PathBuf,
    remote: PathBuf,
}

impl Project {
    fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("project");
        let remote = tmp.path().join("remote.git");
        std::fs::create_dir_all(&repo).unwrap();
        git_ok(&repo, &["init", "-q", "--bare", remote.to_str().unwrap()]);
        git_ok(&repo, &["init", "-q", "-b", "main", "."]);
        git_ok(&repo, &["config", "user.email", "dev@example.com"]);
        git_ok(&repo, &["config", "user.name", "dev"]);
        git_ok(
            &repo,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );

        std::fs::create_dir_all(repo.join("specs")).unwrap();
        std::fs::write(repo.join("src.txt"), "base\n").unwrap();
        std::fs::write(
            repo.join("specs/project.md"),
            "# Project\n\n## Agent Development\n\n### kdevkit\n\
             - `gates:`\n  - `dev:`\n    - `quality: true`\n    - `tests: true`\n",
        )
        .unwrap();
        git_ok(&repo, &["add", "-A"]);
        git_ok(&repo, &["commit", "-q", "-m", "chore: init project"]);
        git_ok(&repo, &["push", "-q", "origin", "main"]);

        Project {
            _tmp: tmp,
            repo,
            remote,
        }
    }

    fn path(&self) -> &Path {
        &self.repo
    }

    /// What a session does to begin a feature: install the hooks, branch,
    /// write the spec, commit it.
    fn start_feature(&self, branch: &str, plan: &[&str]) {
        phase_ok(self.path(), &["install"]);
        git_ok(self.path(), &["checkout", "-q", "-b", branch]);
        self.write_spec(branch, plan);
        git_ok(self.path(), &["add", "-A"]);
        git_ok(
            self.path(),
            &["commit", "-q", "-m", "plan(f): write the spec"],
        );
    }

    fn write_spec(&self, branch: &str, plan: &[&str]) {
        let dir = self.repo.join("specs/feature");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("f.md"),
            format!(
                "# Feature\n\nBranch: {branch}\n\n## Requirements\n\n- R1. it works\n\n\
                 ## Handoff\n\n- **Ready for:** next\n- **Carry forward:** nothing\n\n\
                 ## Implementation Plan\n\n{}\n",
                plan.join("\n")
            ),
        )
        .unwrap();
    }

    fn commit(&self, msg: &str, content: &str) {
        std::fs::write(self.repo.join("src.txt"), format!("{content}\n")).unwrap();
        git_ok(self.path(), &["add", "-A"]);
        let (code, out) = git(self.path(), &["commit", "-q", "-m", msg]);
        assert_eq!(code, 0, "commit {msg:?} was refused:\n{out}");
    }

    /// The stage a reader finds on the branch.
    fn stage(&self) -> String {
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

    fn returns(&self) -> usize {
        let (_, out) = git(
            self.path(),
            &[
                "log",
                "--format=%(trailers:key=Kdevkit-Feature-Return,valueonly=true,unfold=true)",
            ],
        );
        out.lines().filter(|l| !l.trim().is_empty()).count()
    }

    /// Every stage the branch has recorded, oldest first.
    fn stage_history(&self) -> Vec<String> {
        let (_, out) = git(
            self.path(),
            &[
                "log",
                "--reverse",
                "--format=%(trailers:key=Kdevkit-Feature-Stage,valueonly=true,unfold=true)",
            ],
        );
        out.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect()
    }

    fn remote_has(&self, branch: &str) -> bool {
        let (_, refs) = git(self.path(), &["ls-remote", "origin"]);
        refs.contains(branch)
    }

    /// Commit messages on a branch as seen from a fresh clone of the remote.
    fn clone_and_read(&self, branch: &str) -> String {
        let dest = self
            .remote
            .parent()
            .unwrap()
            .join(format!("clone-{branch}"));
        let _ = std::fs::remove_dir_all(&dest);
        let (code, out) = git(
            self.remote.parent().unwrap(),
            &[
                "clone",
                "-q",
                self.remote.to_str().unwrap(),
                dest.to_str().unwrap(),
            ],
        );
        assert_eq!(code, 0, "clone failed:\n{out}");
        // The bare remote's HEAD defaults to `master`, so a clone of a repo
        // whose work is on `main` checks out nothing. Read the branch by name.
        let (code, log) = git(&dest, &["log", "--format=%B", &format!("origin/{branch}")]);
        assert_eq!(
            code, 0,
            "reading origin/{branch} in the clone failed:\n{log}"
        );
        log
    }
}

/// One fact's value, as a reader would see it.
fn fact_of(dir: &Path, key: &str) -> String {
    let (_, out) = phase(dir, &["facts"]);
    out.lines()
        .find_map(|l| l.strip_prefix(&format!("{key}=")))
        .unwrap_or("")
        .trim()
        .to_string()
}

// ── 1 · starting a feature ───────────────────────────────────────

#[test]
fn starting_a_feature_sets_everything_up_and_leaves_other_branches_alone() {
    let p = Project::new();
    let (_, main_before) = git(p.path(), &["rev-parse", "main"]);

    p.start_feature("feat/quiet", &["- [ ] 1 · add the flag"]);

    // The branch, the spec, the hooks and the first recorded stage all exist.
    let (_, branch) = git(p.path(), &["rev-parse", "--abbrev-ref", "HEAD"]);
    assert_eq!(branch.trim(), "feat/quiet");
    assert!(p.path().join("specs/feature/f.md").exists(), "spec missing");
    let (code, hooks) = git(p.path(), &["config", "--get", "core.hooksPath"]);
    assert_eq!(code, 0, "hooks were not wired");
    assert!(hooks.contains("kdevkit"), "wrong hooks path: {hooks}");
    assert_eq!(
        p.stage(),
        "planning",
        "the first commit must record planning"
    );

    // And main is exactly where it was, with nothing stamped on it.
    let (_, main_after) = git(p.path(), &["rev-parse", "main"]);
    assert_eq!(
        main_before, main_after,
        "starting a feature must not move main"
    );
    let (_, main_log) = git(p.path(), &["log", "main", "--format=%B"]);
    assert!(
        !main_log.contains("Kdevkit-Feature-Stage"),
        "main must carry no stage:\n{main_log}"
    );
}

#[test]
fn a_second_feature_does_not_disturb_the_first() {
    let p = Project::new();
    p.start_feature("feat/one", &["- [ ] 1 · one"]);
    p.commit("feat: one", "one");
    let first_stage = p.stage();

    git_ok(p.path(), &["checkout", "-q", "main"]);
    p.start_feature("feat/two", &["- [ ] 1 · two"]);
    assert_eq!(p.stage(), "planning", "the new feature starts at planning");

    // The first feature's record is untouched.
    git_ok(p.path(), &["checkout", "-q", "feat/one"]);
    assert_eq!(
        p.stage(),
        first_stage,
        "one feature's stage must not follow from another's"
    );
}

// ── 2 · the sunny path, all five stages ──────────────────────────

/// Drive a feature from research to closed, taking every step in order,
/// and assert the branch records the whole journey.
#[test]
fn a_feature_goes_all_the_way_through_and_records_every_stage() {
    let p = Project::new();

    // Research first — optional, and its exit is an ack rather than a check.
    phase_ok(p.path(), &["install"]);
    git_ok(p.path(), &["checkout", "-q", "-b", "feat/quiet"]);
    p.write_spec("feat/quiet", &["- [ ] 1 · add the flag"]);
    phase_ok(p.path(), &["advance", "--to", "research", "--ack", "human"]);
    git_ok(p.path(), &["add", "-A"]);
    git_ok(p.path(), &["commit", "-q", "-m", "docs(f): research notes"]);
    assert_eq!(p.stage(), "research");

    // research -> planning
    phase_ok(p.path(), &["advance", "--to", "planning", "--ack", "human"]);
    p.commit("plan(f): the spec", "planned");
    assert_eq!(p.stage(), "planning");

    // planning -> dev
    phase_ok(p.path(), &["advance", "--to", "dev", "--ack", "human"]);
    p.commit("feat(f): first cut", "v1");
    assert_eq!(p.stage(), "dev");

    // The dev loop turns several times; the stage must not move.
    p.commit("fix(f): make the tests pass", "v2");
    p.commit("refactor(f): tidy", "v3");
    assert_eq!(p.stage(), "dev", "the inner loop must not change the stage");
    assert_eq!(p.returns(), 0, "the inner loop must not read as thrash");

    // Leaving dev needs the plan ticked and the checks observed.
    p.write_spec("feat/quiet", &["- [x] 1 · add the flag"]);
    git_ok(p.path(), &["add", "-A"]);
    git_ok(p.path(), &["commit", "-q", "-m", "docs(f): tick the plan"]);
    phase_ok(p.path(), &["verify"]);
    phase_ok(p.path(), &["advance", "--to", "review", "--ack", "human"]);
    p.commit("test(f): cover it", "v4");
    assert_eq!(p.stage(), "review");

    // Publishing runs the checks again, on what is actually being pushed.
    let (code, out) = git(p.path(), &["push", "origin", "feat/quiet"]);
    assert_eq!(code, 0, "a green branch must publish:\n{out}");
    assert!(p.remote_has("feat/quiet"));

    // review -> closure, then closed.
    phase_ok(
        p.path(),
        &["advance", "--to", "closure", "--ack", "session:parent-1"],
    );
    p.commit("chore(f): closure", "v5");
    assert_eq!(p.stage(), "closure");
    phase_ok(p.path(), &["advance", "--to", "closed", "--ack", "human"]);
    p.commit("chore(f): close it out", "v6");
    assert_eq!(p.stage(), "closed");

    // The whole journey is legible from history alone, in order.
    let history = p.stage_history();
    let first_of_each: Vec<&String> = {
        let mut seen = Vec::new();
        for s in &history {
            if seen.last().map(|l: &&String| *l != s).unwrap_or(true) {
                seen.push(s);
            }
        }
        seen
    };
    assert_eq!(
        first_of_each
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>(),
        vec!["research", "planning", "dev", "dev", "dev", "review", "closure", "closed"]
            .into_iter()
            .collect::<Vec<_>>()
            .into_iter()
            .fold(Vec::new(), |mut acc: Vec<&str>, s| {
                if acc.last() != Some(&s) {
                    acc.push(s);
                }
                acc
            }),
        "the stage sequence must read research, planning, dev, review, closure, closed:\n{history:?}"
    );

    // Who approved each move is on the record, including a parent session.
    let (_, log) = git(p.path(), &["log", "--format=%B"]);
    assert!(
        log.contains("Kdevkit-Feature-Ack: human"),
        "human acks missing:\n{log}"
    );
    assert!(
        log.contains("Kdevkit-Feature-Ack: session:parent-1"),
        "a parent session's ack must be attributed:\n{log}"
    );
}

#[test]
fn skipping_the_dev_gates_stops_the_feature_leaving_dev() {
    let p = Project::new();
    p.start_feature("feat/quiet", &["- [ ] 1 · add the flag"]);
    phase_ok(p.path(), &["advance", "--to", "dev"]);
    p.commit("feat(f): work", "v1");

    // Try to leave dev with the plan unticked and nothing verified.
    let (code, out) = phase(p.path(), &["advance", "--to", "review"]);
    assert_ne!(code, 0, "review must be refused:\n{out}");
    assert!(out.contains("unticked"), "must name the plan items:\n{out}");
    assert_eq!(p.stage(), "dev", "the stage must not have moved");

    // Tick the plan but still skip verification.
    p.write_spec("feat/quiet", &["- [x] 1 · add the flag"]);
    git_ok(p.path(), &["add", "-A"]);
    git_ok(p.path(), &["commit", "-q", "-m", "docs(f): tick"]);
    let (code, out) = phase(p.path(), &["advance", "--to", "review"]);
    assert_ne!(code, 0, "still refused without verification:\n{out}");
    assert!(
        out.contains("gates have not been observed passing"),
        "must name the unmet condition:\n{out}"
    );
    assert!(
        out.contains("to resolve:"),
        "a refusal must name the way forward:\n{out}"
    );

    // Verify, and only then is it allowed.
    phase_ok(p.path(), &["verify"]);
    phase_ok(p.path(), &["advance", "--to", "review"]);
}

#[test]
fn a_feature_cannot_short_circuit_the_sequence() {
    let p = Project::new();
    // Everything a later stage could ask for is already true: the plan is
    // ticked, the branch is published. So only the ordering itself can be
    // what refuses these jumps.
    p.start_feature("feat/quiet", &["- [x] 1 · x"]);
    git_ok(
        p.path(),
        &["push", "-q", "--no-verify", "origin", "feat/quiet"],
    );
    phase_ok(p.path(), &["verify"]);

    for skip_to in ["review", "closure", "closed"] {
        let (code, out) = phase(p.path(), &["advance", "--to", skip_to]);
        assert_ne!(
            code, 0,
            "planning -> {skip_to} skips stages and must be refused:\n{out}"
        );
        assert!(
            out.contains("not a forward move"),
            "the refusal must name the ordering as the reason:\n{out}"
        );
        assert_eq!(p.stage(), "planning", "the stage must not have moved");
    }

    // The one legal next step still works, so this is ordering and not a
    // blanket refusal.
    phase_ok(p.path(), &["advance", "--to", "dev"]);
}

// ── 3 · loopy paths ──────────────────────────────────────────────

/// Review finds a requirement-level fault: two stages back, not one.
#[test]
fn review_can_send_work_back_two_stages_and_the_feature_recovers() {
    let p = Project::new();
    p.start_feature("feat/quiet", &["- [x] 1 · add the flag"]);
    phase_ok(p.path(), &["advance", "--to", "dev"]);
    p.commit("feat(f): implement", "v1");
    phase_ok(p.path(), &["verify"]);
    phase_ok(p.path(), &["advance", "--to", "review"]);
    p.commit("test(f): cover it", "v2");
    assert_eq!(p.stage(), "review");

    // The requirement was wrong, so the fault entered at planning — two
    // stages back from review, skipping dev.
    phase_ok(
        p.path(),
        &[
            "return",
            "--to",
            "planning",
            "--fault-entered",
            "requirements",
            "--issue",
            "warning suppression was never specified",
            "--expected-fix",
            "amend R1 and extend the tests",
            "--acceptance",
            "warnings are suppressed with the flag",
        ],
    );
    p.commit("plan(f): record the return", "v3");
    assert_eq!(
        p.stage(),
        "planning",
        "the feature must be back at planning"
    );
    assert_eq!(p.returns(), 1, "the return must be counted");

    // Nothing may move until the recorded problem is dealt with.
    let (code, out) = phase(p.path(), &["advance", "--to", "dev"]);
    assert_ne!(code, 0, "an undischarged return must block:\n{out}");

    // Do the planning work; that discharges it.
    p.write_spec(
        "feat/quiet",
        &["- [x] 1 · add the flag", "- [ ] 2 · suppress warnings"],
    );
    git_ok(p.path(), &["add", "-A"]);
    git_ok(p.path(), &["commit", "-q", "-m", "plan(f): amend R1"]);

    // Forward again through dev, and the new plan item must be honoured.
    phase_ok(p.path(), &["advance", "--to", "dev"]);
    p.commit("feat(f): suppress warnings", "v4");
    let (code, out) = phase(p.path(), &["advance", "--to", "review"]);
    assert_ne!(code, 0, "the new plan item is unticked, so refuse:\n{out}");

    p.write_spec(
        "feat/quiet",
        &["- [x] 1 · add the flag", "- [x] 2 · suppress warnings"],
    );
    git_ok(p.path(), &["add", "-A"]);
    git_ok(p.path(), &["commit", "-q", "-m", "docs(f): tick item 2"]);
    phase_ok(p.path(), &["verify"]);
    phase_ok(p.path(), &["advance", "--to", "review"]);
    p.commit("test(f): cover suppression", "v5");
    assert_eq!(p.stage(), "review", "the feature must reach review again");
    assert_eq!(p.returns(), 1, "one journey back, still counted once");
}

#[test]
fn repeated_returns_are_visible_as_a_count() {
    let p = Project::new();
    p.start_feature("feat/quiet", &["- [x] 1 · x"]);
    phase_ok(p.path(), &["advance", "--to", "dev"]);
    p.commit("feat(f): work", "v1");

    for i in 0..3 {
        phase_ok(
            p.path(),
            &[
                "return",
                "--to",
                "planning",
                "--fault-entered",
                "requirements",
                "--issue",
                "wrong again",
                "--expected-fix",
                "rethink",
                "--acceptance",
                "right this time",
            ],
        );
        p.commit(&format!("plan(f): return {i}"), &format!("r{i}"));
        p.commit(&format!("plan(f): fix {i}"), &format!("f{i}"));
        phase_ok(p.path(), &["advance", "--to", "dev"]);
        p.commit(&format!("feat(f): retry {i}"), &format!("t{i}"));
    }

    assert_eq!(p.returns(), 3, "three trips back must read as three");
    let out = phase_ok(p.path(), &["show"]);
    assert!(
        out.contains("returns=3"),
        "thrash must be visible at a glance:\n{out}"
    );
}

// ── 4 · long sessions and side quests ────────────────────────────

/// A long feature with unrelated work interleaved: the sort of session
/// where a prose instruction gets forgotten. The record must not drift.
#[test]
fn a_long_session_with_side_quests_does_not_lose_the_stage() {
    let p = Project::new();
    p.start_feature("feat/quiet", &["- [ ] 1 · add the flag"]);
    phase_ok(p.path(), &["advance", "--to", "dev"]);

    for i in 0..12 {
        p.commit(&format!("feat(f): step {i}"), &format!("v{i}"));

        // Every few steps, wander off: fix something unrelated on main.
        if i % 4 == 3 {
            git_ok(p.path(), &["checkout", "-q", "main"]);
            std::fs::write(p.path().join("unrelated.txt"), format!("{i}\n")).unwrap();
            git_ok(p.path(), &["add", "-A"]);
            git_ok(p.path(), &["commit", "-q", "-m", "chore: unrelated fix"]);
            let (_, msg) = git(p.path(), &["log", "-1", "--format=%B"]);
            assert!(
                !msg.contains("Kdevkit-Feature-Stage"),
                "a side quest on main must not be stamped:\n{msg}"
            );
            git_ok(p.path(), &["checkout", "-q", "feat/quiet"]);
        }

        assert_eq!(
            p.stage(),
            "dev",
            "the stage must survive step {i} and any detour"
        );
    }

    // After all that, a session that knows nothing still finds the truth.
    let out = phase_ok(p.path(), &["show"]);
    assert!(out.contains("stage=dev"), "{out}");
    assert_eq!(p.returns(), 0, "side quests must not read as returns");
}

#[test]
fn a_side_quest_on_another_feature_branch_does_not_cross_contaminate() {
    let p = Project::new();
    p.start_feature("feat/quiet", &["- [x] 1 · x"]);
    phase_ok(p.path(), &["advance", "--to", "dev"]);
    p.commit("feat(f): work", "v1");
    phase_ok(p.path(), &["verify"]);
    phase_ok(p.path(), &["advance", "--to", "review"]);
    p.commit("test(f): cover", "v2");
    assert_eq!(p.stage(), "review");

    // Wander onto an unrelated chore branch and work there.
    git_ok(p.path(), &["checkout", "-q", "main"]);
    git_ok(p.path(), &["checkout", "-q", "-b", "chore/tidy"]);
    std::fs::write(p.path().join("tidy.txt"), "x\n").unwrap();
    git_ok(p.path(), &["add", "-A"]);
    git_ok(p.path(), &["commit", "-q", "-m", "chore: tidy up"]);
    let (_, msg) = git(p.path(), &["log", "-1", "--format=%B"]);
    assert!(
        !msg.contains("Kdevkit-Feature-Stage"),
        "chore branch stamped:\n{msg}"
    );

    // Back to the feature: still exactly where we left it.
    git_ok(p.path(), &["checkout", "-q", "feat/quiet"]);
    assert_eq!(p.stage(), "review", "the feature's stage must be intact");
}

#[test]
fn a_resumed_session_needs_nothing_but_the_repository() {
    let p = Project::new();
    p.start_feature("feat/quiet", &["- [ ] 1 · x", "- [ ] 2 · y"]);
    phase_ok(p.path(), &["advance", "--to", "dev"]);
    p.commit("feat(f): partial", "v1");

    // Simulate a new session: read everything from the repository.
    let out = phase_ok(p.path(), &["show"]);
    assert!(out.contains("stage=dev"), "stage not recovered:\n{out}");
    assert!(
        out.contains("plan_items_open=2"),
        "outstanding work not recovered:\n{out}"
    );
    assert!(
        out.contains("checks_verified=no"),
        "a resumed session must not inherit a verification it did not do:\n{out}"
    );
}

// ── two features in flight ───────────────────────────────────────

#[test]
fn two_features_in_separate_worktrees_do_not_share_verification() {
    // Verification evidence used to live in git config, which is shared
    // across worktrees — so one feature verifying clobbered the other's
    // record. Parallel features are only possible via worktrees, so this is
    // exactly the case that broke.
    let p = Project::new();
    p.start_feature("feat/one", &["- [x] 1 · one"]);
    p.commit("feat(one): work", "one");

    // A second worktree on its own feature branch.
    let wt = p.path().parent().unwrap().join("wt-two");
    git_ok(
        p.path(),
        &[
            "worktree",
            "add",
            "-q",
            wt.to_str().unwrap(),
            "-b",
            "feat/two",
            "main",
        ],
    );
    phase_ok(&wt, &["install"]);
    let dir = wt.join("specs/feature");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("g.md"),
        "# G\n\n- Branch: `feat/two`\n\n## Handoff\n\n- **Ready for:** dev\n\n         ## Implementation Plan\n\n- [x] 1 · two\n",
    )
    .unwrap();
    git_ok(&wt, &["add", "-A"]);
    git_ok(&wt, &["commit", "-q", "-m", "plan(two): spec"]);

    // Verify feature one, then feature two.
    phase_ok(p.path(), &["verify"]);
    let one_before = fact_of(p.path(), "verified_tree");
    assert!(!one_before.is_empty(), "feature one should be verified");
    phase_ok(&wt, &["verify"]);

    // Feature one's record must be untouched by feature two verifying.
    let one_after = fact_of(p.path(), "verified_tree");
    assert_eq!(
        one_before, one_after,
        "one feature verifying must not overwrite another's record"
    );
    assert_ne!(
        fact_of(&wt, "verified_tree"),
        one_after,
        "the two features cover different trees, so the records must differ"
    );
    assert_eq!(fact_of(p.path(), "checks_verified"), "yes");
    assert_eq!(fact_of(&wt, "checks_verified"), "yes");
}

#[test]
fn each_worktree_keeps_its_own_stage() {
    let p = Project::new();
    p.start_feature("feat/one", &["- [x] 1 · one"]);
    phase_ok(p.path(), &["advance", "--to", "dev"]);
    p.commit("feat(one): work", "one");
    assert_eq!(p.stage(), "dev");

    let wt = p.path().parent().unwrap().join("wt-three");
    // From main, the way starting a feature does — branching off another
    // feature would legitimately inherit its history.
    git_ok(
        p.path(),
        &[
            "worktree",
            "add",
            "-q",
            wt.to_str().unwrap(),
            "-b",
            "feat/three",
            "main",
        ],
    );
    let dir = wt.join("specs/feature");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("h.md"),
        "# H\n\n- Branch: `feat/three`\n\n## Handoff\n\n- **Ready for:** dev\n",
    )
    .unwrap();
    git_ok(&wt, &["add", "-A"]);
    git_ok(&wt, &["commit", "-q", "-m", "plan(three): spec"]);

    let (_, out) = phase(&wt, &["show"]);
    assert!(
        out.contains("stage=planning"),
        "the new feature starts at planning regardless of the other:\n{out}"
    );
    assert_eq!(p.stage(), "dev", "the first feature is unaffected");
}

// ── what reaches main ────────────────────────────────────────────

#[test]
fn an_authored_squash_message_keeps_stage_lines_out_of_a_fresh_clone() {
    let p = Project::new();
    p.start_feature("feat/quiet", &["- [x] 1 · x"]);
    phase_ok(p.path(), &["advance", "--to", "dev"]);
    p.commit("feat(f): implement", "v1");
    phase_ok(p.path(), &["verify"]);
    phase_ok(p.path(), &["advance", "--to", "review"]);
    p.commit("test(f): cover", "v2");

    // Closure squashes with an authored summary rather than the default,
    // which would copy every branch message in.
    git_ok(p.path(), &["checkout", "-q", "main"]);
    git_ok(p.path(), &["merge", "--squash", "-q", "feat/quiet"]);
    git_ok(
        p.path(),
        &[
            "commit",
            "-q",
            "--no-verify",
            "-m",
            "feat(quiet): add a --quiet flag\n\nSuppresses routine output. Requirement R1.",
        ],
    );
    git_ok(p.path(), &["push", "-q", "origin", "main"]);

    // Read it as a newcomer would: from a fresh clone.
    let log = p.clone_and_read("main");
    assert!(
        log.contains("add a --quiet flag"),
        "the summary must survive:\n{log}"
    );
    for leak in [
        "Kdevkit-Feature-Stage",
        "Kdevkit-Feature-Ack",
        "Return-To",
        "Squashed commit",
    ] {
        assert!(
            !log.contains(leak),
            "'{leak}' must not reach a fresh clone of main:\n{log}"
        );
    }
}

#[test]
fn the_default_squash_message_would_leak_and_is_therefore_not_used() {
    // Guards the reason step 1 of the plan exists: this is what happens
    // without an authored message, so if someone later drops that
    // requirement, this test explains what breaks.
    let p = Project::new();
    p.start_feature("feat/quiet", &["- [x] 1 · x"]);
    phase_ok(p.path(), &["advance", "--to", "dev"]);
    p.commit("feat(f): implement", "v1");

    git_ok(p.path(), &["checkout", "-q", "main"]);
    git_ok(p.path(), &["merge", "--squash", "-q", "feat/quiet"]);
    let squash_msg = std::fs::read_to_string(p.path().join(".git/SQUASH_MSG")).unwrap();
    assert!(
        squash_msg.contains("Kdevkit-Feature-Stage"),
        "git's default squash message does carry stage lines — the authored \
         summary is what prevents that, so this must stay true:\n{squash_msg}"
    );
}
