//! Closing and reflecting on a session, deciding the batch, and a habit
//! reaching a second clone of the store, through the binary under a bare
//! environment, with a stub in place of the reflector (its findings,
//! fixed) and the real memory server the profile ships
//! (`MAID_TEST_PENSIEVE`, set by the dev shell).
//!
//! The fixture session: the person asks for a flag, refuses an edit,
//! corrects the agent ("use tr, not sed"), teaches writing-style a rule,
//! which the skill saves to the persona's memory, and the agent fetches a
//! style guide. The HOME also holds an older writing-style rules file.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use tempfile::TempDir;

const SESSION: &str = "0b7c6a8e-1111-4222-8333-944455556666";
const CORE: &str = env!("CARGO_BIN_EXE_maid-core");

fn pensieve() -> String {
    std::env::var("MAID_TEST_PENSIEVE")
        .expect("MAID_TEST_PENSIEVE is not set: run the tests in the dev shell (nix develop)")
}

fn content() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../content")
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        let t = to.join(e.file_name());
        match e.file_type().unwrap().is_dir() {
            true => copy(&e.path(), &t),
            false => {
                fs::copy(e.path(), t).unwrap();
            }
        }
    }
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.org",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

struct Home {
    dir: TempDir,
}

impl Home {
    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn store(&self) -> PathBuf {
        self.path().join("store")
    }

    fn transcript(&self) -> PathBuf {
        self.path()
            .join(format!(".claude/projects/-work/{SESSION}.jsonl"))
    }

    /// What writing-style does on "new rule:": the memory tool call in the
    /// session's transcript, and the memory file and index line the server
    /// leaves in the store.
    fn save_rule(&self, slug: &str, rule: &str) {
        let body = format!(
            "---\nname: \"writing-style: {rule}\"\ndescription: \"{rule}\"\nmetadata:\n  type: feedback\n  bring-in: writing-style\n---\n\n{rule}\n"
        );
        self.save(
            &format!("writing-style-{slug}"),
            &body,
            &format!("writing-style: {rule}"),
            rule,
        );
    }

    /// What the agent does on its own when the memory server's instructions
    /// say to save a correction: the same, with no tag.
    fn save_correction(&self) {
        self.save(
            "use-tr-not-sed",
            "---\nname: Use tr, not sed\ndescription: shell scripts use tr, not sed\nmetadata:\n  type: feedback\n---\n\nIn shell scripts, use tr for case changes; never sed.\n",
            "Use tr, not sed",
            "shell scripts use tr, not sed",
        );
    }

    fn save(&self, file: &str, body: &str, name: &str, hook: &str) {
        let path = format!("/memories/{file}.md");
        let id = format!("toolu_{file}");
        let lines = [
            json!({"type": "assistant", "isSidechain": false, "sessionId": SESSION, "cwd": "/work",
                "message": {"role": "assistant", "content": [{"type": "tool_use", "id": id,
                    "name": "mcp__personal-memory__memory_create",
                    "input": {"path": path, "file_text": body}}]}}),
            json!({"type": "user", "isSidechain": false, "sessionId": SESSION, "cwd": "/work",
                "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": id,
                    "content": format!("Created {path}")}]}}),
        ];
        let mut t = fs::read_to_string(self.transcript()).unwrap();
        for l in lines {
            t.push_str(&format!("{l}\n"));
        }
        fs::write(self.transcript(), t).unwrap();
        let dir = self.store().join("memory");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("{file}.md")), body).unwrap();
        let index = dir.join("MEMORY.md");
        let mut i = fs::read_to_string(&index).unwrap_or_default();
        i.push_str(&format!("- [{name}]({file}.md) - {hook}\n"));
        fs::write(index, i).unwrap();
    }

    /// A HOME with mAId's part (its memory on a store that is a clone of a
    /// bare remote, writing-style, the borrowed document skills, the
    /// adapter), a second part with its own memory, and the fixture
    /// session under Claude Code's projects dir.
    fn new() -> Home {
        let dir = TempDir::new().unwrap();
        let h = dir.path();
        copy(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/claude"),
            &h.join(".claude/projects"),
        );
        let remote = h.join("remote.git");
        fs::create_dir_all(&remote).unwrap();
        git(&remote, &["init", "--bare", "-q"]);
        git(h, &["clone", "-q", remote.to_str().unwrap(), "store"]);
        fs::write(h.join("store/about.md"), "What agents keep about me.\n").unwrap();
        git(&h.join("store"), &["add", "-A"]);
        git(&h.join("store"), &["commit", "-qm", "start"]);
        git(&h.join("store"), &["push", "-q", "origin", "HEAD"]);
        fs::create_dir_all(h.join(".config/maid/writing-style")).unwrap();
        fs::write(
            h.join(".config/maid/writing-style/learned.md"),
            "## Learned rules\n\n- 2026-05-22 · prefer \"to\" over \"in order to\"\n  · source: explicit teach\n",
        )
        .unwrap();
        let adapter = h.join("units/claude-plugin.json");
        fs::create_dir_all(adapter.parent().unwrap()).unwrap();
        let decl: Value = serde_json::from_str(
            &fs::read_to_string(content().join("adapters/claude/my-harness.json")).unwrap(),
        )
        .unwrap();
        fs::write(
            &adapter,
            json!({"name": "maid", "extensions": {"my-harness": decl}}).to_string(),
        )
        .unwrap();
        let work_store = h.join("work-store");
        fs::create_dir_all(&work_store).unwrap();
        let work_server = h.join("units/work-memory.json");
        fs::write(
            &work_server,
            json!({"name": "work-memory", "_meta": {"my-harness": {"scope": "session", "job": "work memory",
                "config": {"store": {"about": "s"}, "index": {"about": "i"}, "serve": {"about": "c"}},
                "state": {"permanent": "store", "promote": "off"}}}})
            .to_string(),
        )
        .unwrap();
        let project =
            json!({"kind": "project", "at": "https://example.org/me/maid", "rev": "abc123"});
        let store = h.join("store");
        let mine = json!({"format": 1, "part": "maid-harness", "persona": "personal",
        "harnesses": {"claude": [{"kind": "plugin", "name": "maid@maid"}]},
        "bring_ins": [
            {"name": "personal-memory",
             "unit": {"standard": "mcp", "path": content().join("servers/personal-memory/server.json")},
             "source": {"kind": "project", "at": "https://example.org/me/pensieve", "rev": "p1"},
             "values": {"store": store, "index": "memory/MEMORY.md",
                "serve": json!([pensieve(), "--store", store]).to_string()}},
            {"name": "writing-style",
             "unit": {"standard": "skill", "path": content().join("skills/writing-style/SKILL.md")},
             "source": project,
             "values": {"learned": h.join(".config/maid/writing-style/learned.md"),
                "promoted": h.join(".config/maid/writing-style/promoted.md")}},
            {"name": "reflect",
             "unit": {"standard": "skill", "path": content().join("skills/reflect/SKILL.md")},
             "source": project},
            {"name": "document-skills",
             "unit": {"standard": "plugin", "path": content().join("overlays/document-skills/plugin.json")},
             "source": {"kind": "vendor", "at": "https://example.org/vendor/skills", "rev": "v1"},
             "tenure": {"kind": "borrowed", "from": "a vendor", "since": "2026-10-01"}},
            {"name": "claude-adapter", "unit": {"standard": "plugin", "path": adapter}, "source": project}
        ]});
        let theirs = json!({"format": 1, "part": "work-part", "persona": "work", "harnesses": {},
            "bring_ins": [{"name": "work-memory", "unit": {"standard": "mcp", "path": work_server},
                "values": {"store": work_store, "index": "memory/MEMORY.md",
                    "serve": json!([pensieve(), "--store", work_store]).to_string()}}]});
        let parts = h.join(".local/state/my-harness/parts");
        fs::create_dir_all(&parts).unwrap();
        fs::write(parts.join("maid-harness.json"), mine.to_string()).unwrap();
        fs::write(parts.join("work-part.json"), theirs.to_string()).unwrap();
        let home = Home { dir };
        home.save_rule(
            "no-em-dashes",
            "Punctuation: no em dashes in commit messages",
        );
        home
    }

    fn core(&self, args: &[&str], stdin: Option<&str>) -> Output {
        let mut child = Command::new(CORE)
            .args(args)
            .env_clear()
            .env("HOME", self.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        {
            use std::io::Write;
            let mut i = child.stdin.take().unwrap();
            if let Some(s) = stdin {
                i.write_all(s.as_bytes()).unwrap();
            }
        }
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out
    }

    fn text(&self, args: &[&str], stdin: Option<&str>) -> String {
        String::from_utf8(self.core(args, stdin).stdout).unwrap()
    }

    /// Every file under HOME but the core's own state, with its bytes.
    fn snapshot(&self) -> Vec<(PathBuf, Vec<u8>)> {
        let state = self.path().join(".local/state/my-harness");
        let mut out = vec![];
        walk(self.path(), &mut out);
        out.retain(|(p, _)| !p.starts_with(&state));
        out.sort();
        out
    }
}

fn walk(dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        match e.file_type().unwrap().is_dir() {
            true => walk(&p, out),
            false => out.push((p.clone(), fs::read(&p).unwrap_or_default())),
        }
    }
}

/// What a reflector returns for the fixture session: the persona habit
/// from the correction. The taught rule is already in memory, so the core
/// lists it and offers its move into the skill.
fn reflection() -> String {
    json!({
        "findings": [
            {"thing": "maid-harness/personal-memory", "kind": "habit", "tier": "permanent",
             "title": "Use tr, not sed", "type": "feedback",
             "change": "shell scripts use tr, not sed",
             "entry": "In shell scripts, use tr for case changes; never sed.",
             "evidence": [{"quote": "no, use tr, not sed: we never use sed in these scripts"}]}
        ],
        "corrections": [{"thing": "maid-harness/personal-memory"}]
    })
    .to_string()
}

fn pending(home: &Home) -> usize {
    fs::read_dir(home.path().join(".local/state/my-harness/pending"))
        .map(|rd| rd.count())
        .unwrap_or(0)
}

fn reflect(home: &Home, reason: &str) -> String {
    let s = ["--session", SESSION];
    let marked = home.text(
        &[
            &["sleep"][..],
            &s,
            &[
                "--input",
                &json!({"reason": reason, "cwd": "/work"}).to_string(),
            ],
        ]
        .concat(),
        None,
    );
    // A session closed again keeps its first mark and adds the reason.
    assert!(
        marked.starts_with(&format!("session {SESSION} marked for sleep ({reason}")),
        "{marked}"
    );
    let brief = home.text(&[&["adaptation"][..], &s].concat(), None);
    assert!(
        brief.starts_with(&format!("Reflect on session {SESSION}.")),
        "{brief}"
    );
    assert!(brief.contains("maid-harness/personal-memory: "), "{brief}");
    home.text(
        &[&["adaptation"][..], &s, &["--input", "-"]].concat(),
        Some(&reflection()),
    )
}

/// Macro step 3, as ruled: the persona habit from the correction, the rule
/// taught to writing-style shown as saved in memory, and its move into the
/// skill as a backlog item. The other shape, the correction saved during
/// the session, follows.
#[test]
fn close_proposes_one_batch_and_ok_writes_the_habit_and_files_the_move_and_nothing_else() {
    let home = Home::new();
    let before = home.snapshot();
    let batch = reflect(&home, "close");
    for want in [
        "2 to decide",
        "  saved in memory this session (it stays there until you remove it):\n    maid-harness/writing-style, in maid-harness/personal-memory [personal] /memories/writing-style-no-em-dashes.md: \"Punctuation: no em dashes in commit messages\"\n",
        "1. habit, maid-harness/personal-memory memory [personal]: add feedback \"Use tr, not sed\" (shell scripts use tr, not sed): \"In shell scripts",
        "2. backlog item, maid-harness/writing-style (on https://example.org/me/maid): move 2 taught rules into writing-style's text",
    ] {
        assert!(batch.contains(want), "{want:?} in:\n{batch}");
    }
    assert!(
        !batch
            .lines()
            .any(|l| l.starts_with("  ") && l.contains("(answer it")),
        "ok takes both:\n{batch}"
    );
    assert_eq!(
        home.snapshot(),
        before,
        "nothing changes before the person decides"
    );
    let genome = home.text(&["genome"], None);
    assert!(genome.contains("pending: 2 decisions"), "{genome}");

    let applied = home.text(
        &[
            "adaptation",
            "--session",
            SESSION,
            "--input",
            r#"{"decide": "ok"}"#,
        ],
        None,
    );
    assert_eq!(applied.matches("accepted").count(), 2, "{applied}");
    assert_eq!(pending(&home), 0);
    let genome = home.text(&["genome"], None);
    assert!(
        genome.contains("pending: 0 decisions") && !genome.contains("to reflect on"),
        "{genome}"
    );

    let memory = home.store().join("memory/use-tr-not-sed.md");
    let text = fs::read_to_string(&memory).unwrap();
    assert!(
        text.starts_with("---\nname: \"Use tr, not sed\"\n"),
        "{text}"
    );
    let index = fs::read_to_string(home.store().join("memory/MEMORY.md")).unwrap();
    assert!(
        index.ends_with("- [Use tr, not sed](use-tr-not-sed.md) - shell scripts use tr, not sed\n"),
        "{index}"
    );

    let after = home.snapshot();
    let changed: Vec<&PathBuf> = after
        .iter()
        .filter(|f| !before.contains(f))
        .map(|(p, _)| p)
        .collect();
    assert_eq!(
        changed,
        [&home.store().join("memory/MEMORY.md"), &memory],
        "only the habit is written; the saved rule and the older file stay"
    );
    let proposals = home
        .path()
        .join(".local/state/my-harness/proposals/maid-harness-writing-style");
    let files: Vec<_> = fs::read_dir(proposals).unwrap().flatten().collect();
    assert_eq!(files.len(), 1);
    let proposal = fs::read_to_string(files[0].path()).unwrap();
    for want in [
        "- Punctuation: no em dashes in commit messages\n",
        "- prefer \"to\" over \"in order to\"\n",
        "https://example.org/me/maid at abc123",
    ] {
        assert!(proposal.contains(want), "{want:?} in:\n{proposal}");
    }
    assert!(
        proposal.trim_end().ends_with("remove these memories so the rules are not in two places: maid-harness/personal-memory /memories/writing-style-no-em-dashes.md."),
        "{proposal}"
    );

    let state = home.path().join(".local/state/my-harness");
    assert!(
        !state.join(format!("briefed/{SESSION}.json")).exists(),
        "the full copy goes once the batch is decided"
    );
    assert!(state.join(format!("experience/{SESSION}.json")).is_file());
    let ledger = home.text(&["ledger"], None);
    assert!(ledger.contains("findings decided/accepted"), "{ledger}");
    assert!(
        ledger.contains("  maid-harness/writing-style: 1, 0, 0, 0, 0, 0, 1/1"),
        "{ledger}"
    );
    assert!(
        ledger.contains("  tool:Edit: 1, 0, 1, 0, 0, 0, 0/0"),
        "{ledger}"
    );
}

/// Macro step 3, the other shape: the agent saved the correction to the
/// persona's memory during the session. The batch lists it as saved in
/// memory beside the taught rule, the reflector's habit for it is dropped,
/// and "ok" files only the move.
#[test]
fn a_correction_saved_in_the_session_is_listed_and_ok_files_only_the_move() {
    let home = Home::new();
    home.save_correction();
    let before = home.snapshot();
    let batch = reflect(&home, "close");
    for want in [
        "1 to decide",
        "  saved in memory this session (it stays there until you remove it):\n    maid-harness/writing-style, in maid-harness/personal-memory [personal] /memories/writing-style-no-em-dashes.md: \"Punctuation: no em dashes in commit messages\"\n    maid-harness/personal-memory [personal] /memories/use-tr-not-sed.md: \"shell scripts use tr, not sed\"\n",
        "1. backlog item, maid-harness/writing-style (on https://example.org/me/maid): move 2 taught rules into writing-style's text",
        "shell scripts use tr, not sed: saved in memory this session (maid-harness/personal-memory /memories/use-tr-not-sed.md)",
    ] {
        assert!(batch.contains(want), "{want:?} in:\n{batch}");
    }
    assert!(!batch.contains(". habit"), "{batch}");
    let applied = decide(&home, r#"{"decide": "ok"}"#);
    assert_eq!(applied.matches("accepted").count(), 1, "{applied}");
    assert_eq!(pending(&home), 0);
    assert_eq!(
        home.snapshot(),
        before,
        "the saved memories and the older file stay as they are"
    );
    let proposals = home
        .path()
        .join(".local/state/my-harness/proposals/maid-harness-writing-style");
    let files: Vec<_> = fs::read_dir(proposals).unwrap().flatten().collect();
    assert_eq!(files.len(), 1);
    let proposal = fs::read_to_string(files[0].path()).unwrap();
    assert!(!proposal.contains("use-tr-not-sed"), "{proposal}");
}

fn decide(home: &Home, answer: &str) -> String {
    home.text(
        &["adaptation", "--session", SESSION, "--input", answer],
        None,
    )
}

#[test]
fn a_declined_move_is_not_offered_again_until_a_new_rule_is_saved() {
    let home = Home::new();
    let first = reflect(&home, "close");
    assert!(first.contains("2. backlog item"), "{first}");
    decide(&home, r#"{"decide": {"1": "yes", "2": "no"}}"#);

    let again = reflect(&home, "close");
    assert!(
        !again.contains("backlog item") && !again.contains("taught rule"),
        "a second close after the decline offers no move:\n{again}"
    );
    decide(&home, r#"{"decide": "none"}"#);

    home.save_rule("oxford-comma", "Punctuation: Oxford comma, always");
    let third = reflect(&home, "close");
    assert!(
        third.contains("backlog item, maid-harness/writing-style (on https://example.org/me/maid): move 1 taught rule into writing-style's text"),
        "a new rule brings the move back, with only what is new:\n{third}"
    );
    let proposals = home.path().join(".local/state/my-harness/proposals");
    assert!(!proposals.exists(), "a decline files nothing");
}

#[test]
fn a_reflection_started_mid_session_routes_the_same_way() {
    let close = Home::new();
    let mid = Home::new();
    let routes = |home: &Home, reason: &str| -> Vec<Value> {
        reflect(home, reason);
        let out = home.core(
            &[
                "adaptation",
                "--session",
                SESSION,
                "--json",
                "--input",
                r#"{"decide": "ok"}"#,
            ],
            None,
        );
        let v: Value = serde_json::from_slice(&out.stdout).unwrap();
        v["items"][0]["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| json!([o["n"], o["thing"], o["outcome"]]))
            .collect()
    };
    assert_eq!(routes(&mid, "reflect"), routes(&close, "close"));
}

#[test]
fn an_end_event_marks_the_session_and_the_next_genome_says_it_waits() {
    let home = Home::new();
    let end = json!({"session_id": SESSION, "hook_event_name": "SessionEnd", "cwd": "/work", "reason": "other",
        "transcript_path": home.path().join(format!(".claude/projects/-work/{SESSION}.jsonl"))});
    home.text(
        &["sleep", "--carrier", "append", "--input", "-"],
        Some(&end.to_string()),
    );
    let genome = home.text(&["genome"], None);
    assert!(genome.contains("to reflect on: 1 sessions"), "{genome}");
    let waiting = home.text(&["experience"], None);
    assert!(
        waiting.contains(&format!("  {SESSION} (end, ")),
        "{waiting}"
    );
}

#[test]
fn the_habit_travels_once_the_person_syncs_the_store() {
    let home = Home::new();
    reflect(&home, "close");
    home.text(
        &[
            "adaptation",
            "--session",
            SESSION,
            "--input",
            r#"{"decide": {"1": "yes"}}"#,
        ],
        None,
    );
    let store = home.store();
    // The server leaves the store as its own writes do: changed, not
    // committed. Syncing is the person's.
    git(&store, &["add", "-A"]);
    git(&store, &["commit", "-qm", "sync"]);
    git(&store, &["push", "-q", "origin", "HEAD"]);
    let elsewhere = TempDir::new().unwrap();
    git(
        elsewhere.path(),
        &[
            "clone",
            "-q",
            home.path().join("remote.git").to_str().unwrap(),
            "store",
        ],
    );
    let there = elsewhere.path().join("store/memory");
    assert!(fs::read_to_string(there.join("MEMORY.md"))
        .unwrap()
        .contains("[Use tr, not sed](use-tr-not-sed.md)"));
    assert!(there.join("use-tr-not-sed.md").is_file());
}

/// The reflector runs inside the session it reflects on, so by the time
/// its findings come back the transcript holds its own answer. A quote
/// found only there backs nothing.
#[test]
fn a_quote_only_in_what_came_after_the_brief_is_not_evidence() {
    let home = Home::new();
    let s = ["--session", SESSION];
    home.text(
        &[&["sleep"][..], &s, &["--input", r#"{"reason": "reflect"}"#]].concat(),
        None,
    );
    home.text(&[&["adaptation"][..], &s].concat(), None);
    let made_up = "the person always wants everything force-pushed";
    let dir = home
        .path()
        .join(format!(".claude/projects/-work/{SESSION}"));
    fs::write(
        dir.join("subagents/agent-reflector.jsonl"),
        format!(
            "{}\n",
            json!({"type": "assistant", "isSidechain": true,
            "message": {"content": [{"type": "text", "text": made_up}]}})
        ),
    )
    .unwrap();
    let findings = json!({"findings": [{"thing": "maid-harness/personal-memory", "kind": "habit",
        "title": "Force-push", "type": "feedback", "change": "force-push", "entry": made_up,
        "evidence": [{"quote": made_up}]}]});
    let batch = home.text(
        &[&["adaptation"][..], &s, &["--input", "-"]].concat(),
        Some(&findings.to_string()),
    );
    assert!(
        !batch.contains(". habit") && batch.contains("is not in the session"),
        "{batch}"
    );
}

#[test]
fn findings_without_a_brief_are_refused() {
    let home = Home::new();
    let out = Command::new(CORE)
        .args(["adaptation", "--session", SESSION, "--input", &reflection()])
        .env_clear()
        .env("HOME", home.path())
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("get the brief first"));
}

#[test]
fn skip_leaves_the_session_out_and_dismiss_clears_the_waiting_ones() {
    let home = Home::new();
    reflect(&home, "close");
    let out = home.text(
        &[
            "adaptation",
            "--session",
            SESSION,
            "--input",
            r#"{"decide": "skip"}"#,
        ],
        None,
    );
    assert_eq!(out, format!("skipped: nothing from {SESSION} is applied\n"));
    let genome = home.text(&["genome"], None);
    assert!(
        genome.contains("pending: 0 decisions") && !genome.contains("to reflect on"),
        "{genome}"
    );
    assert!(!home.store().join("memory/use-tr-not-sed.md").exists());
    assert!(!home
        .path()
        .join(".local/state/my-harness/proposals")
        .exists());
    let state = home.path().join(".local/state/my-harness");
    assert!(!state.join(format!("briefed/{SESSION}.json")).exists());
    assert!(home
        .text(&["ledger"], None)
        .contains("dismissed without reflection: 1 sessions"));

    let other = Home::new();
    let end = json!({"session_id": SESSION, "hook_event_name": "SessionEnd",
        "transcript_path": other.path().join(format!(".claude/projects/-work/{SESSION}.jsonl"))});
    other.text(&["sleep", "--input", "-"], Some(&end.to_string()));
    assert!(other
        .text(&["genome"], None)
        .contains("to reflect on: 1 sessions"));
    let out = other.text(&["adaptation", "--input", r#"{"dismiss": "all"}"#], None);
    assert_eq!(out, format!("dismissed 1 sessions: {SESSION}\n"));
    assert!(!other.text(&["genome"], None).contains("to reflect on"));
}
