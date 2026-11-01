//! Claude Code as a session source: its transcripts, one JSONL file per
//! session under `<projects>/<project dir>/<session>.jsonl`, each
//! sub-agent in `<session>/subagents/agent-<id>.jsonl`.
//!
//! Claude Code calls the line format internal, so lines are read as plain
//! JSON values and only the fields named here are used: a line of a shape
//! not known is skipped, and one that is not JSON is counted.

use crate::delivery::check_id;
use crate::experience::{Bundle, Did, Entry, Friction, Struggle, Who};
use crate::source::{Located, SessionSource};
use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

pub struct Claude {
    /// The projects dir.
    pub root: PathBuf,
}

/// Claude Code names a project's dir after its working directory with
/// every character that is not a letter or digit turned into `-`.
pub fn project_dir_name(path: &str) -> String {
    path.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

impl SessionSource for Claude {
    fn name(&self) -> &'static str {
        crate::source::DEFAULT
    }

    fn locate(&self, session: Option<&str>, input: &Value) -> Result<Located> {
        if let Some(s) = session {
            check_id("session id", s)?;
        }
        let transcript = match (input["transcript_path"].as_str(), session) {
            (Some(p), _) => self.inside(Path::new(p), session)?,
            (None, Some(s)) => self.by_id(s)?,
            (None, None) => self.newest(input["cwd"].as_str())?,
        };
        let session = stem(&transcript)?;
        check_id("session id", &session)?;
        let subs = list_jsonl(&transcript.with_extension("").join("subagents"))?;
        Ok(Located {
            source: self.name().into(),
            session,
            transcript,
            subs,
        })
    }

    fn pack(&self, at: &Located) -> Result<Bundle> {
        let text = fs::read_to_string(&at.transcript)
            .with_context(|| format!("cannot read {}", at.transcript.display()))?;
        let mut main = vec![];
        let mut side: BTreeMap<String, Vec<(usize, Value)>> = BTreeMap::new();
        let mut skipped = 0;
        for (n, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Value>(line) {
                Ok(v) if v["isSidechain"] == true => {
                    let agent = v["agentId"].as_str().unwrap_or("sidechain").to_string();
                    side.entry(agent).or_default().push((n + 1, v));
                }
                Ok(v) => main.push((n + 1, v)),
                Err(_) => skipped += 1,
            }
        }
        let file = at.transcript.display().to_string();
        let mut bundle = pack_lines(&at.session, &file, main, Who::Person);
        bundle.skipped_lines = skipped;
        for sub in &at.subs {
            let text = fs::read_to_string(sub)
                .with_context(|| format!("cannot read {}", sub.display()))?;
            let mut lines = vec![];
            let mut skipped = 0;
            for (n, line) in text.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<Value>(line) {
                    Ok(v) => lines.push((n + 1, v)),
                    Err(_) => skipped += 1,
                }
            }
            let mut b = pack_lines(&stem(sub)?, &sub.display().to_string(), lines, Who::Agent);
            b.skipped_lines = skipped;
            bundle.sub_sessions.push(b);
        }
        for (agent, lines) in side {
            bundle
                .sub_sessions
                .push(pack_lines(&agent, &file, lines, Who::Agent));
        }
        Ok(bundle)
    }
}

impl Claude {
    /// A transcript a carrier named: a regular file `<session>.jsonl`
    /// inside the projects dir, and nowhere else.
    fn inside(&self, p: &Path, session: Option<&str>) -> Result<PathBuf> {
        let root = fs::canonicalize(&self.root)
            .with_context(|| format!("no Claude Code projects dir at {}", self.root.display()))?;
        let real =
            fs::canonicalize(p).with_context(|| format!("no transcript at {}", p.display()))?;
        if !real.starts_with(&root) || !fs::metadata(&real)?.is_file() {
            bail!(
                "{} is not a transcript under {}",
                p.display(),
                root.display()
            );
        }
        if let Some(s) = session {
            if real.file_name().and_then(|n| n.to_str()) != Some(&format!("{s}.jsonl")) {
                bail!("{} is not session {s}'s transcript", p.display());
            }
        }
        Ok(real)
    }

    fn by_id(&self, session: &str) -> Result<PathBuf> {
        let name = format!("{session}.jsonl");
        let mut dirs: Vec<PathBuf> = fs::read_dir(&self.root)
            .with_context(|| format!("cannot list {}", self.root.display()))?
            .flatten()
            .map(|e| e.path())
            .collect();
        dirs.sort();
        dirs.into_iter()
            .map(|d| d.join(&name))
            .find(|f| f.is_file())
            .ok_or_else(|| {
                anyhow!(
                    "no transcript for session {session} under {}",
                    self.root.display()
                )
            })
    }

    /// With no id: the newest transcript for the working dir.
    fn newest(&self, cwd: Option<&str>) -> Result<PathBuf> {
        let cwd = cwd.ok_or_else(|| anyhow!("name the session, or the working dir it ran in"))?;
        let dir = self.root.join(project_dir_name(cwd));
        list_jsonl(&dir)?
            .into_iter()
            .filter_map(|f| Some((fs::metadata(&f).ok()?.modified().ok()?, f)))
            .max()
            .map(|(_, f)| f)
            .ok_or_else(|| anyhow!("no transcript for {cwd} under {}", self.root.display()))
    }
}

fn stem(p: &Path) -> Result<String> {
    p.file_stem()
        .and_then(|s| s.to_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("{} has no name", p.display()))
}

/// Every `*.jsonl` file in `dir`, in name order; none when it is absent.
fn list_jsonl(dir: &Path) -> Result<Vec<PathBuf>> {
    let rd = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e).with_context(|| format!("cannot list {}", dir.display())),
    };
    let mut out: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jsonl") && p.is_file())
        .collect();
    out.sort();
    Ok(out)
}

const INTERRUPT: &str = "[Request interrupted by user";
const REJECTED: [&str; 2] = [
    "doesn't want to proceed with this tool use",
    "tool use was rejected",
];
const SKILL_BODY: &str = "Base directory for this skill";
/// Text Claude Code puts in the person's turn that the person did not
/// write: a command's output, a background task's result, a reminder.
const COMMAND_OUTPUT: [&str; 6] = [
    "<local-command-stdout>",
    "<local-command-stderr>",
    "<bash-stdout>",
    "<bash-stderr>",
    "<task-notification>",
    "<system-reminder>",
];

struct Packer<'a> {
    file: &'a str,
    /// Who a user turn's text is: the person in a session; in a sub-agent,
    /// the agent that dispatched it, whose prompt may carry anything it
    /// read.
    user: Who,
    b: Bundle,
    touched: BTreeSet<String>,
    /// Each tool call by id: its place in `did` and what it was.
    calls: HashMap<String, (usize, String)>,
    /// Calls (tool and input) that failed, to see one repeated.
    failed: HashSet<String>,
}

fn pack_lines(session: &str, file: &str, lines: Vec<(usize, Value)>, user: Who) -> Bundle {
    let mut p = Packer {
        file,
        user,
        b: Bundle {
            session: session.into(),
            source: crate::source::DEFAULT.into(),
            transcript: file.into(),
            ..Default::default()
        },
        touched: BTreeSet::new(),
        calls: HashMap::new(),
        failed: HashSet::new(),
    };
    for (n, v) in lines {
        p.line(n, &v);
    }
    p.b.touched = p.touched.into_iter().collect();
    p.b
}

/// A tool result's text: a string, or its text blocks joined.
fn text_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(|i| i["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

impl Packer<'_> {
    fn at(&self, n: usize) -> String {
        format!("{}:{n}", self.file)
    }

    fn told(&mut self, n: usize, kind: &str, text: String) {
        // Claude Code repeats some standing text (its output style, say)
        // on every turn; the first copy is what the agent was told.
        if self
            .b
            .told
            .iter()
            .any(|e| e.kind.as_deref() == Some(kind) && e.text == text)
        {
            return;
        }
        let at = self.at(n);
        self.b.told.push(Entry {
            at,
            who: Who::Told,
            kind: Some(kind.into()),
            text,
            truncated: false,
        });
    }

    fn said(&mut self, n: usize, who: Who, text: String) {
        let at = self.at(n);
        self.b.said.push(Entry {
            at,
            who,
            kind: None,
            text,
            truncated: false,
        });
    }

    fn line(&mut self, n: usize, v: &Value) {
        match v["type"].as_str() {
            Some("user") => self.user(n, v),
            Some("assistant") => self.assistant(n, v),
            Some("system") => match v["subtype"].as_str() {
                Some("compact_boundary") => self.told(n, "compaction", "compacted".into()),
                _ => {
                    let t = text_of(&v["content"]);
                    if !t.is_empty() {
                        self.told(n, "system", t);
                    }
                }
            },
            Some("attachment") => {
                let a = &v["attachment"];
                let kind = a["type"].as_str().unwrap_or("attachment").to_string();
                let t = [&a["content"], &a["text"], &a["stdout"]]
                    .into_iter()
                    .map(text_of)
                    .find(|t| !t.is_empty())
                    .unwrap_or_else(|| a.to_string());
                self.told(n, &kind, t);
            }
            _ => {}
        }
    }

    fn user(&mut self, n: usize, v: &Value) {
        let meta = v["isMeta"] == true;
        let content = &v["message"]["content"];
        if v["isCompactSummary"] == true {
            return self.told(n, "compaction", text_of(content));
        }
        let blocks: Vec<Value> = match content {
            Value::String(s) => vec![serde_json::json!({"type": "text", "text": s})],
            Value::Array(a) => a.clone(),
            _ => vec![],
        };
        for block in blocks {
            match block["type"].as_str() {
                Some("text") => {
                    let t = block["text"].as_str().unwrap_or_default().to_string();
                    if meta {
                        let kind = if t.starts_with(SKILL_BODY) {
                            "skill"
                        } else {
                            "meta"
                        };
                        self.told(n, kind, t);
                    } else if t.starts_with(INTERRUPT) {
                        let at = self.at(n);
                        self.b.friction.push(Friction {
                            at,
                            kind: Struggle::Interrupt,
                            tool: None,
                            text: t,
                            truncated: false,
                        });
                    } else if COMMAND_OUTPUT.iter().any(|m| t.starts_with(m)) {
                        // Output of a command the person ran, in their turn:
                        // tool content, not their words.
                        let at = self.at(n);
                        self.b.told.push(Entry {
                            at,
                            who: Who::Tool,
                            kind: Some("command output".into()),
                            text: t,
                            truncated: false,
                        });
                    } else {
                        let who = self.user;
                        self.said(n, who, t);
                    }
                }
                Some("tool_result") => self.result(n, &block),
                _ => {}
            }
        }
    }

    fn result(&mut self, n: usize, block: &Value) {
        let text = text_of(&block["content"]);
        let id = block["tool_use_id"].as_str().unwrap_or_default();
        let call = self.calls.get(id).cloned();
        let tool = call.as_ref().map(|(i, _)| self.b.did[*i].tool.clone());
        if block["is_error"] == true {
            let kind = match REJECTED.iter().any(|r| text.contains(r)) {
                true => Struggle::Refusal,
                false => Struggle::Error,
            };
            if let Some((i, key)) = &call {
                self.failed.insert(key.clone());
                self.b.did[*i].failed = true;
            }
            let at = self.at(n);
            self.b.friction.push(Friction {
                at,
                kind,
                tool: tool.clone(),
                text: text.clone(),
                truncated: false,
            });
        }
        let entry = Entry {
            at: self.at(n),
            who: Who::Tool,
            kind: None,
            text,
            truncated: false,
        };
        match call {
            Some((i, _)) => self.b.did[i].result = Some(entry),
            // A result whose call is not in this file still counts as
            // tool content.
            None => self.b.told.push(Entry {
                kind: Some("tool result".into()),
                ..entry
            }),
        }
    }

    fn assistant(&mut self, n: usize, v: &Value) {
        let Value::Array(blocks) = &v["message"]["content"] else {
            let t = text_of(&v["message"]["content"]);
            if !t.is_empty() {
                self.said(n, Who::Agent, t);
            }
            return;
        };
        for block in blocks {
            match block["type"].as_str() {
                Some("text") => {
                    let t = block["text"].as_str().unwrap_or_default().to_string();
                    if !t.trim().is_empty() {
                        self.said(n, Who::Agent, t);
                    }
                }
                Some("tool_use") => self.call(n, block),
                _ => {}
            }
        }
    }

    fn call(&mut self, n: usize, block: &Value) {
        let tool = block["name"].as_str().unwrap_or("?").to_string();
        let input = &block["input"];
        for key in ["file_path", "path", "notebook_path", "url"] {
            if let Some(p) = input[key].as_str() {
                self.touched.insert(p.to_string());
            }
        }
        if tool == "Skill" {
            if let Some(s) = input["skill"].as_str() {
                self.told(n, "skill", format!("loaded skill {s}"));
            }
        }
        let key = format!("{tool} {input}");
        if self.failed.contains(&key) {
            let at = self.at(n);
            self.b.friction.push(Friction {
                at,
                kind: Struggle::Retry,
                tool: Some(tool.clone()),
                text: input.to_string(),
                truncated: false,
            });
        }
        if let Some(id) = block["id"].as_str() {
            self.calls.insert(id.to_string(), (self.b.did.len(), key));
        }
        let at = self.at(n);
        self.b.did.push(Did {
            at,
            tool,
            input: input.to_string(),
            truncated: false,
            result: None,
            failed: false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    use crate::fixtures::SESSION as S;

    fn fixture() -> (TempDir, Claude) {
        let dir = TempDir::new().unwrap();
        let root = crate::fixtures::claude_projects(dir.path());
        (dir, Claude { root })
    }

    fn packed() -> Bundle {
        let (_d, c) = fixture();
        let at = c.locate(Some(S), &Value::Null).unwrap();
        c.pack(&at).unwrap()
    }

    #[test]
    fn the_persons_words_and_the_agents_text_are_said() {
        let b = packed();
        let person: Vec<&str> = b
            .said
            .iter()
            .filter(|e| e.who == Who::Person)
            .map(|e| e.text.as_str())
            .collect();
        assert_eq!(
            person,
            [
                "add a --upper flag to greet.sh",
                "no, use tr, not sed: we never use sed in these scripts",
                "new rule: no em dashes in commit messages",
                "<command-name>/compact</command-name>",
            ]
        );
        assert!(b
            .said
            .iter()
            .any(|e| e.who == Who::Agent && e.text.contains("I'll read greet.sh")));
    }

    #[test]
    fn skill_bodies_hook_context_and_compaction_are_told() {
        let b = packed();
        let kinds: Vec<&str> = b.told.iter().filter_map(|e| e.kind.as_deref()).collect();
        assert_eq!(
            kinds,
            [
                "hook_additional_context",
                "skill",
                "skill",
                "command output",
                "compaction",
                "compaction"
            ]
        );
        assert!(b.told[2].text.starts_with(SKILL_BODY));
        assert_eq!(
            b.told[3].who,
            Who::Tool,
            "a command's output is not the person's"
        );
        assert!(b
            .told
            .iter()
            .enumerate()
            .all(|(i, e)| i == 3 || e.who == Who::Told));
    }

    #[test]
    fn tool_calls_are_did_with_their_results_and_paths_touched() {
        let b = packed();
        let tools: Vec<&str> = b.did.iter().map(|d| d.tool.as_str()).collect();
        assert_eq!(
            tools,
            ["Read", "Edit", "Skill", "Bash", "Bash", "WebFetch", "Agent"]
        );
        assert_eq!(
            b.did[0].result.as_ref().unwrap().text,
            "1\techo \"hello $1\"\n"
        );
        assert_eq!(b.did[0].result.as_ref().unwrap().who, Who::Tool);
        assert_eq!(
            b.touched,
            ["/work/greet.sh", "https://example.org/style-guide"]
        );
    }

    #[test]
    fn interrupts_refusals_errors_and_a_repeated_failing_call_are_friction() {
        let b = packed();
        let kinds: Vec<(Struggle, Option<&str>)> = b
            .friction
            .iter()
            .map(|f| (f.kind, f.tool.as_deref()))
            .collect();
        assert_eq!(
            kinds,
            [
                (Struggle::Refusal, Some("Edit")),
                (Struggle::Interrupt, None),
                (Struggle::Error, Some("Bash")),
                (Struggle::Retry, Some("Bash")),
                (Struggle::Error, Some("Bash")),
            ]
        );
        let failed: Vec<&str> = b
            .did
            .iter()
            .filter(|d| d.failed)
            .map(|d| d.tool.as_str())
            .collect();
        assert_eq!(
            failed,
            ["Edit", "Bash", "Bash"],
            "the call keeps that it failed"
        );
    }

    #[test]
    fn sub_agents_in_their_own_files_and_inline_become_sub_sessions() {
        let b = packed();
        let subs: Vec<&str> = b.sub_sessions.iter().map(|s| s.session.as_str()).collect();
        assert_eq!(subs, ["agent-a1b2c3", "old-side"]);
        assert_eq!(b.sub_sessions[0].did[0].tool, "Grep");
        assert!(
            b.sub_sessions
                .iter()
                .all(|s| s.said.iter().all(|e| e.who == Who::Agent)),
            "a sub-agent's prompt is its dispatcher's, not the person's"
        );
        assert!(b.sub_sessions[1].said[0].text.contains("side chain"));
        assert!(
            !b.said.iter().any(|e| e.text.contains("side chain")),
            "inline side chains leave the main session"
        );
    }

    #[test]
    fn unknown_types_are_skipped_and_broken_lines_counted() {
        let b = packed();
        assert_eq!(b.skipped_lines, 1);
        assert!(!b.told.iter().any(|e| e.text.contains("snapshot")));
    }

    #[test]
    fn a_session_is_found_by_its_end_event_path_by_id_or_as_the_newest_for_a_dir() {
        let (d, c) = fixture();
        let path = c.root.join(format!("-work/{S}.jsonl"));
        let by_path = c.locate(None, &json!({"transcript_path": path})).unwrap();
        assert_eq!(by_path.session, S);
        assert_eq!(by_path.subs.len(), 1);
        assert_eq!(c.locate(Some(S), &Value::Null).unwrap().transcript, path);
        let newest = c.locate(None, &json!({"cwd": "/work"})).unwrap();
        assert_eq!(newest.session, S);
        let outside = d.path().join(format!("{S}.jsonl"));
        fs::write(&outside, "").unwrap();
        assert!(c
            .locate(None, &json!({"transcript_path": outside}))
            .is_err());
        assert!(c
            .locate(Some("other"), &json!({"transcript_path": path}))
            .is_err());
        assert!(c.locate(Some("../x"), &Value::Null).is_err());
        assert!(c.locate(Some("nope"), &Value::Null).is_err());
        assert!(c.locate(None, &json!({"cwd": "/elsewhere"})).is_err());
    }

    #[test]
    fn project_dirs_are_named_as_claude_code_names_them() {
        assert_eq!(project_dir_name("/a/b.c_d e"), "-a-b-c-d-e");
    }
}
