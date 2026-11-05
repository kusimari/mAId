//! What a session saved in a persona's memory, and the rules a bring-in
//! keeps there, moved into its own text. A bring-in that learns during a
//! session (writing-style's "new rule:") saves to the persona's memory
//! through the memory's own server, tagging the memory
//! `metadata.bring-in: <name>`; the agent may save other memories there on
//! its own. At the batch the core lists every memory this session saved,
//! and offers one item per bring-in with tagged rules: move those rules,
//! and any in its older rules files, into its text, as a proposal on its
//! source. See project.md "My harness", Adaptation.

use super::{id, is_memory, resolve, Evidence, Finding, Kind, Op};
use crate::environment::{contained, index_of};
use crate::experience::{squash, Bundle};
use crate::map::{BringIn, Map};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::{Component, Path};

/// One memory this session saved to a persona's memory.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Saved {
    /// The bring-in a tagged rule is for (`<part>/<name>`); none for a
    /// memory with no tag, or a tag no declared bring-in has.
    pub thing: Option<String>,
    /// The memory it is in (`<part>/<name>`), and its persona.
    pub memory: String,
    pub persona: Option<String>,
    /// The memory tool's path, `/memories/...`.
    pub path: String,
    /// A tagged rule's body; else the memory's description, name or first
    /// line.
    pub rule: String,
    /// Its name and body, which a habit proposed again is matched on.
    #[serde(skip)]
    pub name: Option<String>,
    #[serde(skip)]
    pub body: String,
    /// The tool call that saved it, `<file>:<line>`.
    pub at: String,
}

/// What a move item carries, so a decided move is not offered again: its
/// key, and the memory to remove once it lands.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Move {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
}

/// A memory tool call that writes a memory, on a memory bring-in's own
/// server: `mcp__<name>__<tool>`, or a plugin's `mcp__plugin_<p>_<name>__<tool>`.
fn writes_on(tool: &str, memory: &str) -> bool {
    let Some((server, call)) = tool.strip_prefix("mcp__").and_then(|t| t.split_once("__")) else {
        return false;
    };
    matches!(call, "memory_create" | "memory_str_replace")
        && (server == memory
            || server
                .strip_prefix("plugin_")
                .and_then(|p| p.strip_suffix(memory))
                .is_some_and(|p| p.len() > 1 && p.ends_with('_')))
}

/// Every memory a memory tool call in the session or its sub-sessions
/// saved, that is in its store now.
pub fn saved(map: &Map, bundle: &Bundle) -> Vec<Saved> {
    let mut out: Vec<Saved> = vec![];
    saved_into(map, bundle, &mut out);
    out
}

fn saved_into(map: &Map, bundle: &Bundle, out: &mut Vec<Saved>) {
    for d in bundle.did.iter().filter(|d| !d.failed) {
        let Some(m) = map
            .bring_ins
            .iter()
            .find(|b| is_memory(b) && writes_on(&d.tool, &b.name))
        else {
            continue;
        };
        let Ok(input) = serde_json::from_str::<Value>(&d.input) else {
            continue;
        };
        let Some(path) = input["path"].as_str() else {
            continue;
        };
        let Some(text) = in_store(m, path) else {
            continue;
        };
        let (name, body) = parts(&text);
        let (thing, rule) = match tagged(&text).and_then(|(t, r)| Some((resolve(map, &t)?, r))) {
            Some((b, rule)) => (Some(id(b)), rule),
            None => (None, summary(&text)),
        };
        if out.iter().any(|s| s.path == path && s.memory == id(m)) {
            continue;
        }
        out.push(Saved {
            thing,
            memory: id(m),
            persona: m.persona.clone(),
            path: path.into(),
            rule,
            name,
            body,
            at: d.at.clone(),
        });
    }
    for sub in &bundle.sub_sessions {
        saved_into(map, sub, out);
    }
}

/// A memory file's text, read from the store the server serves as
/// `/memories` (the index's dir), when the path stays in it.
fn in_store(m: &BringIn, path: &str) -> Option<String> {
    let rel = path.strip_prefix("/memories/")?;
    if !Path::new(rel)
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
    {
        return None;
    }
    let (store, index) = index_of(m)?.ok()?;
    let dir = index.parent()?.strip_prefix(&store).ok()?.to_path_buf();
    let file = contained(&store, &dir.join(rel)).ok()?;
    // The index lists memories; it is not one.
    if Some(Path::new(rel).as_os_str()) == index.file_name() {
        return None;
    }
    crate::contract::read_small(&file).ok()
}

/// A memory's `metadata.bring-in` tag and its body.
fn tagged(text: &str) -> Option<(String, String)> {
    let parsed = gray_matter::Matter::<gray_matter::engine::YAML>::new()
        .parse::<Value>(text)
        .ok()?;
    let tag = parsed.data?["metadata"]["bring-in"].as_str()?.to_string();
    let body = parsed.content.trim().to_string();
    (!body.is_empty()).then_some((tag, body))
}

/// A memory's frontmatter and body.
fn split(text: &str) -> (Value, String) {
    match gray_matter::Matter::<gray_matter::engine::YAML>::new().parse::<Value>(text) {
        Ok(p) => (p.data.unwrap_or_default(), p.content),
        Err(_) => (Value::Null, text.to_string()),
    }
}

/// A memory's name and trimmed body.
fn parts(text: &str) -> (Option<String>, String) {
    let (data, body) = split(text);
    (
        data["name"].as_str().map(str::to_string),
        body.trim().to_string(),
    )
}

/// What an untagged memory says in one line: its description, its name,
/// or its body's first line.
fn summary(text: &str) -> String {
    let (data, body) = split(text);
    ["description", "name"]
        .iter()
        .filter_map(|k| data[*k].as_str())
        .chain(body.lines())
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or_default()
        .to_string()
}

/// One older rules file: its path, its key, and its rules.
struct Older {
    path: String,
    key: String,
    rules: Vec<String>,
}

/// An own bring-in's older rules files: each bound config value that is
/// not a state tier and is a file in the tier shape, `- ` entries under a
/// `## ` heading. A rule is its entry line without the date lead, with the
/// indented lines under it. A borrowed bring-in's files are never moved.
fn older(b: &BringIn) -> Vec<Older> {
    let own = b
        .record
        .as_ref()
        .is_some_and(|r| r.tenure == crate::parts::Tenure::Own);
    let Some(d) = b.declaration().filter(|_| own) else {
        return vec![];
    };
    let tiers: BTreeSet<&str> = d.state.iter().flat_map(|s| s.keys()).collect();
    let Some(values) = b.record.as_ref().map(|r| &r.values) else {
        return vec![];
    };
    let mut out = vec![];
    for key in d.config.keys().filter(|k| !tiers.contains(k.as_str())) {
        let Some(at) = values.get(key) else {
            continue;
        };
        if !Path::new(at).is_absolute() {
            continue;
        }
        let Ok(text) = crate::contract::read_small(Path::new(at)) else {
            continue;
        };
        if !text.lines().any(|l| l.starts_with("## ")) {
            continue;
        }
        let (mut rules, mut open) = (Vec::<String>::new(), false);
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("- ") {
                let rest = match rest.split_once(" · ") {
                    Some((date, r)) if is_date(date) => r,
                    _ => rest,
                };
                rules.push(rest.trim().to_string());
                open = true;
            } else if open && line.starts_with("  ") && !line.trim().is_empty() {
                // Its example stays with it; where it came from does not
                // belong in the skill's text.
                if !line.trim().starts_with("· source:") {
                    if let Some(last) = rules.last_mut() {
                        let _ = write!(last, "\n{}", line.trim());
                    }
                }
            } else if !line.trim().is_empty() {
                open = false;
            }
        }
        if rules.is_empty() {
            continue;
        }
        out.push(Older {
            key: format!("file:{at}#{:016x}", crate::delivery::fnv1a(&text)),
            path: at.clone(),
            rules,
        });
    }
    out
}

fn is_date(s: &str) -> bool {
    s.len() == 10 && s.as_bytes()[4] == b'-' && s.as_bytes()[7] == b'-'
}

fn rule_key(thing: &str, rule: &str) -> String {
    format!("rule:{thing}:{}", squash(rule).to_lowercase())
}

/// The move items: one per bring-in with rules to move that no decided or
/// waiting move already holds (`held`). Each is a nature finding on the
/// bring-in carrying the text to add, and the keys it moves. An older file
/// is read again only once it changed, and then gives only its new rules.
pub fn items(map: &Map, saved: &[Saved], held: &BTreeSet<String>) -> Vec<(Finding, Vec<Move>)> {
    let mut out = vec![];
    for b in map.bring_ins.iter().filter(|b| b.declaration().is_some()) {
        let thing = id(b);
        let mine: Vec<&Saved> = saved
            .iter()
            .filter(|s| {
                s.thing.as_deref() == Some(thing.as_str())
                    && !held.contains(&rule_key(&thing, &s.rule))
            })
            .collect();
        let files: Vec<Older> = older(b)
            .into_iter()
            .filter(|o| !held.contains(&o.key))
            .map(|mut o| {
                o.rules.retain(|r| !held.contains(&rule_key(&thing, r)));
                o
            })
            .filter(|o| !o.rules.is_empty())
            .collect();
        if mine.is_empty() && files.is_empty() {
            continue;
        }
        let (mut edit, mut evidence, mut moves, mut n) = (String::new(), vec![], vec![], 0);
        let _ = writeln!(
            edit,
            "Add each rule to {}'s text, under the section it names:\n",
            b.name
        );
        for s in &mine {
            n += 1;
            let _ = writeln!(edit, "- {}", indent(&s.rule));
            let _ = writeln!(edit, "  (saved in memory: {} {})", s.memory, s.path);
            evidence.push(Evidence {
                quote: first_line(&s.rule),
                at: Some(s.at.clone()),
            });
            moves.push(Move {
                key: rule_key(&thing, &s.rule),
                memory: Some(format!("{} {}", s.memory, s.path)),
            });
        }
        for o in &files {
            for r in &o.rules {
                n += 1;
                let _ = writeln!(edit, "- {}", indent(r));
                let _ = writeln!(edit, "  (older file: {})", o.path);
                evidence.push(Evidence {
                    quote: first_line(r),
                    at: Some(o.path.clone()),
                });
                moves.push(Move {
                    key: rule_key(&thing, r),
                    memory: None,
                });
            }
            moves.push(Move {
                key: o.key.clone(),
                memory: None,
            });
        }
        let what = match n {
            1 => "1 taught rule".to_string(),
            n => format!("{n} taught rules"),
        };
        out.push((
            Finding {
                thing: Some(thing),
                kind: Kind::Nature,
                tier: None,
                op: Op::Add,
                persona: None,
                guess: false,
                title: None,
                memory_type: None,
                change: format!("move {what} into {}'s text", b.name),
                entry: None,
                edit: Some(edit.trim_end().to_string()),
                evidence,
                contradicts: None,
                route: None,
            },
            moves,
        ));
    }
    out
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or_default().trim().to_string()
}

/// A rule's later lines, indented under its bullet.
fn indent(s: &str) -> String {
    s.lines().map(str::trim).collect::<Vec<_>>().join("\n  ")
}

/// The proposal's last line: the memories to remove once it lands.
pub fn remove_line(moves: &[Move]) -> Option<String> {
    let memories: Vec<&str> = moves.iter().filter_map(|m| m.memory.as_deref()).collect();
    (!memories.is_empty()).then(|| {
        format!(
            "Once this lands, remove these memories so the rules are not in two places: {}.",
            memories.join(", ")
        )
    })
}
