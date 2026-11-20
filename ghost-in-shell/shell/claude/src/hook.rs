//! Claude Code's own input, turned into the call `infusion` takes. The
//! field names here are Claude's (its hook payloads, and the same JSON a
//! person passes the binary), and this file is the only place that knows
//! them: everything behind it reads a call, not a shell.

use anyhow::{bail, Result};
use infusion::call::{sub_session, Close, Where};
use serde_json::{Map, Value};
use std::path::PathBuf;

/// What a carrier's own JSON says about a call.
#[derive(Debug, Default, PartialEq)]
pub struct Input {
    pub session: Option<String>,
    pub at: Where,
    pub prompt: Option<String>,
    pub close: Option<Close>,
    /// The person's own input, passed through: the reflector's findings
    /// and the person's decisions.
    pub own: Value,
}

/// The keys that are the person's own, not this shell's: exactly what
/// `infusion::adaptation` reads of a call's input. A key it learns to read
/// has to be added here too, or it never reaches it.
const OWN: [&str; 4] = ["decide", "dismiss", "findings", "corrections"];

/// The keys that say where the session is. A value of another type than a
/// string is refused rather than dropped, since dropping it would answer
/// for a call the carrier did not make.
const WHERE: [&str; 3] = ["source", "transcript_path", "cwd"];

/// Read Claude's JSON. What it does not say is left unset; what it says in
/// a shape this shell cannot take is an error.
pub fn read(input: &Value) -> Result<Input> {
    let text = |k: &str| input.get(k).and_then(Value::as_str);
    let path = |k: &str| text(k).map(PathBuf::from);
    for k in WHERE {
        if !input[k].is_null() && text(k).is_none() {
            bail!("{k} must be a string (got {})", input[k]);
        }
    }
    let own: Map<String, Value> = OWN
        .iter()
        .filter(|k| !input[**k].is_null())
        .map(|k| (k.to_string(), input[*k].clone()))
        .collect();
    Ok(Input {
        session: session(input),
        at: Where {
            source: text("source").map(str::to_string),
            transcript: path("transcript_path"),
            cwd: path("cwd"),
        },
        prompt: text("prompt").map(str::to_string),
        // Claude's end event carries its own reason (`other`, `clear`,
        // ...); that the session ended is what the mark keeps.
        close: match (text("hook_event_name"), text("reason")) {
            (Some("SessionEnd"), _) => Some(Close::End),
            (_, Some(r)) => Some(Close::Named(r.to_string())),
            (_, None) => None,
        },
        own: match own.is_empty() {
            true => Value::Null,
            false => Value::Object(own),
        },
    })
}

/// The session Claude's input names: its `session_id`, and with an
/// `agent_id` that sub-session's own id.
fn session(input: &Value) -> Option<String> {
    let session = input.get("session_id")?.as_str()?;
    match input.get("agent_id").and_then(Value::as_str) {
        None => Some(session.to_string()),
        Some(agent) => Some(sub_session(session, agent)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn the_session_comes_from_the_input_and_a_sub_agent_gets_its_own() {
        let from = |v: Value| read(&v).unwrap().session;
        assert_eq!(from(json!({"session_id": "s1"})).as_deref(), Some("s1"));
        assert_eq!(
            from(json!({"session_id": "s1", "agent_id": "a7"})).as_deref(),
            Some("s1.a7")
        );
        let long = from(json!({"session_id": "s1", "agent_id": "x/".repeat(80)})).unwrap();
        assert!(long.starts_with("s1.a-"), "{long}");
        assert_eq!(from(json!({"agent_id": "a7"})), None);
        assert_eq!(from(json!({"session_id": 3})), None);
        assert_eq!(from(Value::Null), None);
    }

    #[test]
    fn a_start_a_prompt_and_an_end_event_each_read_as_the_call_they_are() {
        let start = read(
            &json!({"session_id": "s", "hook_event_name": "SessionStart",
            "source": "startup", "cwd": "/work"}),
        )
        .unwrap();
        assert_eq!(start.at.cwd.as_deref(), Some(Path::new("/work")));
        assert_eq!(
            start.at.source.as_deref(),
            Some("startup"),
            "a source the shell names is passed on; the sources say what they serve"
        );
        assert_eq!((start.prompt, start.close), (None, None));
        let turn = read(&json!({"session_id": "s", "prompt": "deploy", "cwd": "/work"})).unwrap();
        assert_eq!(turn.prompt.as_deref(), Some("deploy"));
        let end = read(&json!({"session_id": "s", "hook_event_name": "SessionEnd",
            "reason": "other", "transcript_path": "/t/s.jsonl"}))
        .unwrap();
        assert_eq!(end.close, Some(Close::End));
        assert_eq!(end.at.transcript.as_deref(), Some(Path::new("/t/s.jsonl")));
        let close = read(&json!({"reason": "reflect"})).unwrap();
        assert_eq!(close.close, Some(Close::Named("reflect".into())));
    }

    #[test]
    fn only_the_persons_own_keys_are_passed_through() {
        let got = read(&json!({"session_id": "s", "cwd": "/w", "decide": "ok",
            "findings": [], "nonsense": 1}))
        .unwrap();
        assert_eq!(got.own, json!({"decide": "ok", "findings": []}));
        assert_eq!(read(&json!({"cwd": "/w"})).unwrap().own, Value::Null);
    }

    /// A location of another type is refused, so a call that named one
    /// badly is never answered as a call that named none.
    #[test]
    fn a_location_that_is_not_a_string_is_refused() {
        for k in WHERE {
            let e = read(&json!({ k: 5 })).unwrap_err();
            assert!(
                e.to_string() == format!("{k} must be a string (got 5)"),
                "{e}"
            );
        }
        assert!(read(&json!({"cwd": Value::Null})).unwrap().at.cwd.is_none());
        assert!(
            read(&json!({"session_id": 3})).is_ok(),
            "an id that is not a string names no session, as it did before"
        );
    }

    /// infusion knows no shell: it never names one of Claude's own fields
    /// and takes no CLI crate. This shell is where both belong.
    #[test]
    fn infusion_names_no_field_of_this_shell_and_no_cli_crate() {
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../infusion");
        // Each as it would be written to read the field, so a test's
        // own name is not a hit.
        let claude = [
            "\"session_id\"",
            "\"agent_id\"",
            "\"transcript_path\"",
            "\"hook_event_name\"",
            "\"SessionEnd\"",
            "[\"cwd\"]",
            "[\"prompt\"]",
            "[\"source\"]",
        ];
        let mut seen = 0;
        for file in rust_files(&crate_dir.join("src")) {
            let text = std::fs::read_to_string(&file).unwrap();
            seen += 1;
            for name in claude {
                assert!(
                    !text.contains(name),
                    "{} names {name}, which is this shell's",
                    file.display()
                );
            }
        }
        assert!(seen > 10, "only {seen} files read under infusion/src");
        let manifest = std::fs::read_to_string(crate_dir.join("Cargo.toml")).unwrap();
        assert!(!manifest.contains("clap"), "infusion depends on clap");
    }

    fn rust_files(dir: &Path) -> Vec<PathBuf> {
        let mut out = vec![];
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            match p.is_dir() {
                true => out.extend(rust_files(&p)),
                false if p.extension().is_some_and(|x| x == "rs") => out.push(p),
                false => {}
            }
        }
        out
    }
}
