//! `infusion`'s entry points over this shell's own session source: an end
//! event and a close mark the session, experience packs it, and the genome
//! counts what waits. Spans crates, so it is here and not in either.

use ghost::config::Config;
use ghost::environment::Host;
use infusion::call::{Call, Carrier, Close, Ctx, Where};
use infusion::genome::{picture, render};
use infusion::{experience, sleep};
use serde_json::json;
use shell_claude::transcript::Transcripts;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use ghost_in_shell_tests::{claude_projects, SESSION as S};

/// A context over `<dir>`, reading sessions from the fixture projects dir
/// copied to `<dir>/claude`.
fn ctx(dir: &Path, projects: Option<PathBuf>) -> Ctx {
    Ctx {
        config: Config {
            parts: dir.join("parts"),
            state: dir.join("state"),
        },
        host: Host::default(),
        sources: Box::new(Transcripts { projects }),
    }
}

fn call(session: Option<&str>, carrier: Carrier) -> Call {
    Call {
        session: session.map(str::to_string),
        carrier,
        ..Call::default()
    }
}

fn part(dir: &Path) {
    let parts = dir.join("parts");
    fs::create_dir_all(&parts).unwrap();
    let v = json!({"format": 1, "part": "a", "harnesses": {}, "persona": "personal",
        "bring_ins": []});
    fs::write(parts.join("a.json"), v.to_string()).unwrap();
}

#[test]
fn an_end_event_marks_the_session_and_the_genome_counts_it_waiting() {
    let dir = TempDir::new().unwrap();
    let projects = claude_projects(dir.path());
    let c = ctx(dir.path(), Some(projects.clone()));
    part(dir.path());
    let end = Call {
        close: Some(Close::End),
        at: Where {
            cwd: Some("/work".into()),
            transcript: Some(projects.join(format!("-work/{S}.jsonl"))),
            ..Where::default()
        },
        ..call(Some(S), Carrier::Append)
    };
    assert!(
        sleep(&c, &end).unwrap().is_empty(),
        "an ended session is told nothing"
    );
    let p = picture(&c).unwrap();
    assert_eq!(p.waiting, 1);
    assert!(render(&p).contains("  to reflect on: 1 sessions marked for sleep"));
    let listed = experience(&c, &call(None, Carrier::OnDemand)).unwrap();
    assert!(
        listed[0].text.contains(&format!("  {S} (end, ")),
        "{}",
        listed[0].text
    );
}

#[test]
fn a_close_names_the_session_and_experience_packs_and_stores_it() {
    let dir = TempDir::new().unwrap();
    let projects = claude_projects(dir.path());
    let c = ctx(dir.path(), Some(projects));
    let close = Call {
        at: Where {
            cwd: Some("/work".into()),
            ..Where::default()
        },
        ..call(Some(S), Carrier::OnDemand)
    };
    assert!(sleep(&c, &close).unwrap()[0].text.ends_with("(close)\n"));
    let got = experience(&c, &call(Some(S), Carrier::OnDemand)).unwrap();
    let stored = dir.path().join(format!("state/experience/{S}.json"));
    assert!(stored.is_file());
    assert!(
        got[0].text.contains("(4 by the person)")
            && got[0]
                .text
                .contains("friction: 1 interrupt, 1 refusal, 2 error, 1 retry")
            && got[0].text.contains("2 sub-sessions"),
        "{}",
        got[0].text
    );
    let none = experience(&c, &call(None, Carrier::OnDemand)).unwrap();
    assert!(none[0].text.contains("sessions to reflect on: 1"));
}

#[test]
fn a_source_not_built_says_so() {
    let dir = TempDir::new().unwrap();
    let k = Call {
        at: Where {
            source: Some("pi".into()),
            cwd: Some("/w".into()),
            ..Where::default()
        },
        ..call(Some("s"), Carrier::OnDemand)
    };
    let e = sleep(&ctx(dir.path(), None), &k).unwrap_err();
    assert!(
        e.to_string().contains("no session source for pi yet"),
        "{e}"
    );
}

#[test]
fn with_no_claude_dir_a_session_cannot_be_read() {
    let dir = TempDir::new().unwrap();
    let e = sleep(&ctx(dir.path(), None), &call(Some(S), Carrier::OnDemand)).unwrap_err();
    assert!(
        e.to_string().contains("no Claude Code dir"),
        "{e}: HOME and CLAUDE_CONFIG_DIR unset leaves no transcripts"
    );
}
