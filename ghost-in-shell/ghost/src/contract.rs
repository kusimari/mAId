//! The bring-in contract: what a bring-in declares about itself, carried in
//! its standard's own extension slot under the `my-harness` namespace, so
//! the unit stays a plain unit of its standard. See project.md "My harness".
//!
//! - an Agent Skill: frontmatter `metadata`, a flat string map, so each
//!   field is its own `my-harness.<field>` key;
//! - a plugin manifest: `extensions["my-harness"]`, a JSON object;
//! - an MCP server manifest (`server.json`): `_meta["my-harness"]`, the same.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub const NAMESPACE: &str = "my-harness";

/// The contract version this ghost reads. A declaration of any other, or
/// of none, is refused.
pub const CONTRACT: u64 = 1;

/// The standard a unit follows, which says where its slot is.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Standard {
    Skill,
    Plugin,
    Mcp,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    Session,
    Turn,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct Signals {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outcomes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub jobs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub locations: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
}

/// One config key a bring-in needs bound per environment or part.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct Key {
    #[serde(default)]
    pub about: String,
    /// Its value is never held, only where it comes from (`env:<NAME>`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub secret: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub optional: bool,
    /// Where its value comes from where the unit runs: `arg:<flag>` (the
    /// registered argument after the flag), `command` (the registered
    /// command and arguments), `env:<NAME>` (the registered environment),
    /// `value:<text>` or `path:<template>` (`$NAME`, `${NAME:-default}`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Promote {
    Auto,
    #[default]
    Propose,
    Off,
}

/// Which config keys hold the bring-in's local and permanent memory.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
pub struct State {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permanent: Option<String>,
    #[serde(default)]
    pub promote: Promote,
}

impl State {
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.local.iter().chain(&self.permanent).map(String::as_str)
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Declaration {
    /// The contract version it follows.
    pub contract: u64,
    /// The bring-in's name, where its standard's name is not it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub scope: Scope,
    pub job: String,
    #[serde(default)]
    pub signals: Signals,
    /// A pointer to the few lines that come first while it applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub config: BTreeMap<String, Key>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<State>,
    /// Commands that must be on PATH for it to be ready.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
    /// Where the work's own record is, for a turn bring-in that knows:
    /// `<file glob>#<heading slug>`, each relative to the session's location.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub record: Vec<String>,
    /// On a memory bring-in only: the persona it remembers for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona: Option<String>,
    /// Where a change to it is proposed. On a plugin, its standard
    /// `repository` field when the slot names none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
}

/// What a unit's slot holds.
#[derive(Serialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "lowercase", tag = "is")]
pub enum Slot {
    Declared(Box<Declaration>),
    /// No declaration: the standard's own description stands in for `job`.
    Undeclared {
        description: Option<String>,
    },
}

/// Read the declaration from the unit at `path`. `Err` says why the unit or
/// its declaration could not be read.
pub fn read(standard: Standard, path: &Path) -> Result<Slot, String> {
    if !path.is_absolute() {
        return Err(format!("{} is not an absolute path", path.display()));
    }
    let text = read_small(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    match standard {
        Standard::Skill => from_skill(&text),
        Standard::Plugin => from_manifest(&text, "extensions"),
        Standard::Mcp => from_manifest(&text, "_meta"),
    }
}

/// The lines a `rules` pointer names, read from the unit at `unit`. A
/// pointer is `<file>#<anchor>`: `file` relative to the unit's dir (never
/// absolute, never through `..`), `anchor` a heading's slug as GitHub makes
/// it. The rules are that heading's section, up to the next heading of the
/// same or a higher level; headings inside fenced code do not count.
pub fn rules(unit: &Path, pointer: &str) -> Result<String, String> {
    let (file, anchor) = pointer
        .split_once('#')
        .filter(|(f, a)| !f.is_empty() && !a.is_empty())
        .ok_or_else(|| format!("rules {pointer:?} is not <file>#<anchor>"))?;
    let rel = Path::new(file);
    if rel.is_absolute()
        || rel
            .components()
            .any(|c| c == std::path::Component::ParentDir)
    {
        return Err(format!("rules {pointer:?} must stay in the unit's dir"));
    }
    let path = unit.parent().unwrap_or(Path::new("/")).join(rel);
    section(&path, anchor)
}

/// The section of the markdown file at `path` under the heading whose slug
/// is `anchor`, as `rules` reads it.
pub fn section(path: &Path, anchor: &str) -> Result<String, String> {
    let text = read_small(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    // The open fence's marker and length: only a run of the same marker,
    // at least as long, closes it.
    let mut fence: Option<(char, usize)> = None;
    let mut section: Option<(usize, Vec<&str>)> = None;
    for line in text.lines() {
        let heading = fence.is_none().then(|| heading(line)).flatten();
        if let Some((c, n)) = fence_of(line) {
            match fence {
                None => fence = Some((c, n)),
                Some((open, len)) if c == open && n >= len => fence = None,
                Some(_) => {}
            }
        }
        match (&mut section, heading) {
            (Some((level, _)), Some((l, _))) if l <= *level => break,
            (Some((_, lines)), _) => lines.push(line),
            (None, Some((l, t))) if slug(t) == anchor => section = Some((l, vec![line])),
            (None, _) => {}
        }
    }
    let (_, lines) =
        section.ok_or_else(|| format!("no heading #{anchor} in {}", path.display()))?;
    Ok(lines.join("\n").trim_end().to_string())
}

/// Up to three leading spaces, which markdown still reads as column 0.
fn unindent(line: &str) -> Option<&str> {
    let indent = line.bytes().take_while(|b| *b == b' ').count();
    (indent <= 3).then(|| &line[indent..])
}

/// A fence line's marker and run length: three or more backticks or
/// tildes.
fn fence_of(line: &str) -> Option<(char, usize)> {
    let t = unindent(line)?;
    let c = t.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let n = t.chars().take_while(|x| *x == c).count();
    (n >= 3).then_some((c, n))
}

/// `## Title` (closing `#`s dropped) as (2, "Title"); anything else is not
/// a heading.
fn heading(line: &str) -> Option<(usize, &str)> {
    let line = unindent(line)?;
    let level = line.bytes().take_while(|b| *b == b'#').count();
    let rest = line.get(level..)?;
    if !(1..=6).contains(&level) || !(rest.is_empty() || rest.starts_with(' ')) {
        return None;
    }
    let title = rest.trim();
    let closed = title.trim_end_matches('#');
    match closed.is_empty() || closed.ends_with(' ') {
        true => Some((level, closed.trim_end())),
        false => Some((level, title)),
    }
}

/// GitHub's heading anchor: lowercase, only letters, digits, spaces, `-`
/// and `_` kept, each space a `-`.
pub fn slug(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
        .map(|c| if c == ' ' { '-' } else { c })
        .collect()
}

/// A manifest is a small regular file. Anything else (a FIFO, a device, a
/// huge file) would stall or swell the hook that reads it.
const MAX_MANIFEST: u64 = 1 << 20;

/// A file read whole, when it is a regular file of at most 1 MiB.
pub fn read_small(path: &Path) -> std::io::Result<String> {
    use std::io::{Error, Read};
    if !fs::metadata(path)?.is_file() {
        return Err(Error::other("not a regular file"));
    }
    let mut text = String::new();
    fs::File::open(path)?
        .take(MAX_MANIFEST + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAX_MANIFEST {
        return Err(Error::other("larger than 1 MiB"));
    }
    Ok(text)
}

fn from_manifest(text: &str, slot_name: &str) -> Result<Slot, String> {
    let v: Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
    let description = v["description"].as_str().map(str::to_string);
    let slot = match &v[slot_name][NAMESPACE] {
        Value::Null => return Ok(Slot::Undeclared { description }),
        decl => parse(decl.clone())?,
    };
    if slot_name != "extensions" {
        return Ok(slot);
    }
    // A plugin's standard `repository` field, a URL or `{ "url" }`.
    let standard = v["repository"]
        .as_str()
        .or_else(|| v["repository"]["url"].as_str());
    Ok(match (slot, standard) {
        (Slot::Declared(mut d), Some(r)) if d.repository.is_none() => {
            d.repository = Some(r.into());
            Slot::Declared(d)
        }
        (slot, _) => slot,
    })
}

fn from_skill(text: &str) -> Result<Slot, String> {
    #[derive(Deserialize)]
    struct Front {
        description: Option<String>,
        #[serde(default)]
        metadata: Option<BTreeMap<String, Value>>,
    }
    let front = gray_matter::Matter::<gray_matter::engine::YAML>::new()
        .parse::<Front>(text)
        .map_err(|e| format!("frontmatter: {e}"))?
        .data
        .ok_or("no frontmatter")?;
    let prefix = format!("{NAMESPACE}.");
    let ours: Vec<(String, Value)> = front
        .metadata
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(k, v)| k.strip_prefix(&prefix).map(|f| (f.to_string(), v)))
        .collect();
    if ours.is_empty() {
        return Ok(Slot::Undeclared {
            description: front.description,
        });
    }
    let mut flat = BTreeMap::new();
    for (field, v) in ours {
        let Value::String(s) = v else {
            return Err(format!("metadata {prefix}{field} is not a string"));
        };
        flat.insert(field, s);
    }
    parse(unflatten(&flat))
}

/// The flat skill keys as the object a plugin or server manifest carries.
/// Keys this reader does not know are dropped, as the object form ignores
/// unknown fields.
fn unflatten(flat: &BTreeMap<String, String>) -> Value {
    let list = |s: &str| -> Value {
        s.split(',')
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .map(|x| Value::String(x.into()))
            .collect()
    };
    let (mut obj, mut signals, mut config, mut state) =
        (Map::new(), Map::new(), Map::new(), Map::new());
    for (field, value) in flat {
        let (head, rest) = field.split_once('.').unwrap_or((field, ""));
        match (head, rest) {
            ("scope" | "job" | "rules" | "package" | "persona" | "repository" | "name", "") => {
                obj.insert(head.into(), json!(value));
            }
            ("contract", "") => {
                obj.insert(
                    head.into(),
                    value.parse::<u64>().map_or(json!(value), |n| json!(n)),
                );
            }
            ("requires" | "record", "") => {
                obj.insert(head.into(), list(value));
            }
            ("signals", kind) if !kind.is_empty() => {
                signals.insert(kind.into(), list(value));
            }
            ("config", key) if !key.is_empty() => {
                let mut k = key_of(value);
                if let Some(b) = flat.get(&format!("bind.{key}")) {
                    k["bind"] = json!(b);
                }
                config.insert(key.into(), k);
            }
            ("state", part) if !part.is_empty() => {
                state.insert(part.into(), json!(value));
            }
            _ => {}
        }
    }
    for (k, m) in [("signals", signals), ("config", config), ("state", state)] {
        if !m.is_empty() {
            obj.insert(k.into(), Value::Object(m));
        }
    }
    Value::Object(obj)
}

/// `"[secret] [optional] about"`: leading bracketed markers, then the text.
fn key_of(value: &str) -> Value {
    let (mut secret, mut optional, mut rest) = (false, false, value.trim_start());
    loop {
        if let Some(r) = rest.strip_prefix("[secret]") {
            (secret, rest) = (true, r.trim_start());
        } else if let Some(r) = rest.strip_prefix("[optional]") {
            (optional, rest) = (true, r.trim_start());
        } else {
            break;
        }
    }
    json!({ "about": rest.trim_end(), "secret": secret, "optional": optional })
}

fn parse(decl: Value) -> Result<Slot, String> {
    if !decl.is_object() {
        return Err(format!("the {NAMESPACE} declaration is not an object"));
    }
    match &decl["contract"] {
        Value::Null => return Err(format!("the {NAMESPACE} declaration names no contract")),
        Value::Number(n) if n.as_u64() == Some(CONTRACT) => {}
        other => {
            let why = format!("follows contract {other}, which this ghost does not know");
            return Err(format!("the {NAMESPACE} declaration {why}"));
        }
    }
    let d: Declaration =
        serde_json::from_value(decl).map_err(|e| format!("the {NAMESPACE} declaration: {e}"))?;
    if let Some(state) = &d.state {
        if let Some(k) = state.keys().find(|k| !d.config.contains_key(*k)) {
            return Err(format!("state names {k:?}, which is not a config key"));
        }
    }
    Ok(Slot::Declared(Box::new(d)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(dir: &TempDir, name: &str, text: &str) -> std::path::PathBuf {
        let p = dir.path().join(name);
        fs::write(&p, text).unwrap();
        p
    }

    fn declared(slot: Result<Slot, String>) -> Declaration {
        match slot {
            Ok(Slot::Declared(d)) => *d,
            other => panic!("{other:?}"),
        }
    }

    /// One declaration, written in each slot's own form, reads the same.
    fn full() -> Declaration {
        Declaration {
            contract: 1,
            name: None,
            scope: Scope::Session,
            job: "memory for the personal persona".into(),
            signals: Signals {
                jobs: vec!["recall".into(), "save".into()],
                ..Signals::default()
            },
            rules: Some("SKILL.md#rules".into()),
            package: Some("pensieve".into()),
            config: BTreeMap::from([
                (
                    "store".into(),
                    Key {
                        about: "the persona's store".into(),
                        bind: Some("arg:--store".into()),
                        ..Key::default()
                    },
                ),
                (
                    "token".into(),
                    Key {
                        about: "a sync token".into(),
                        secret: true,
                        optional: true,
                        bind: None,
                    },
                ),
            ]),
            state: Some(State {
                local: Some("store".into()),
                permanent: None,
                promote: Promote::Auto,
            }),
            requires: vec!["git".into(), "python".into()],
            record: vec!["specs/*.md#handoff".into(), "docs/*.md#handoff".into()],
            persona: Some("personal".into()),
            repository: Some("https://example.org/me/kit".into()),
        }
    }

    #[test]
    fn a_skill_declares_in_flat_metadata_keys() {
        let dir = TempDir::new().unwrap();
        let p = write(
            &dir,
            "SKILL.md",
            "---\nname: s\ndescription: d\nmetadata:\n  other.key: kept out\n  my-harness.contract: \"1\"\n  my-harness.persona: personal\n  my-harness.repository: https://example.org/me/kit\n  my-harness.bind.store: arg:--store\n  my-harness.scope: session\n  my-harness.job: memory for the personal persona\n  my-harness.signals.jobs: recall, save\n  my-harness.rules: SKILL.md#rules\n  my-harness.package: pensieve\n  my-harness.config.store: the persona's store\n  my-harness.config.token: \"[secret] [optional] a sync token\"\n  my-harness.state.local: store\n  my-harness.state.promote: auto\n  my-harness.requires: git, python\n  my-harness.record: specs/*.md#handoff, docs/*.md#handoff\n  my-harness.future: ignored\n---\nbody\n",
        );
        assert_eq!(declared(read(Standard::Skill, &p)), full());
    }

    #[test]
    fn a_plugin_and_a_server_declare_in_their_manifest_slot() {
        let dir = TempDir::new().unwrap();
        let decl = serde_json::to_value(full()).unwrap();
        let plugin = write(
            &dir,
            "plugin.json",
            &json!({"name": "p", "extensions": {"my-harness": decl}}).to_string(),
        );
        let server = write(
            &dir,
            "server.json",
            &json!({"name": "s", "_meta": {"my-harness": decl, "other": 1}}).to_string(),
        );
        assert_eq!(declared(read(Standard::Plugin, &plugin)), full());
        assert_eq!(declared(read(Standard::Mcp, &server)), full());
    }

    #[test]
    fn a_plugins_standard_repository_stands_when_its_slot_names_none() {
        let dir = TempDir::new().unwrap();
        let mut decl = serde_json::to_value(full()).unwrap();
        decl.as_object_mut().unwrap().remove("repository");
        let plugin = write(
            &dir,
            "plugin.json",
            &json!({"name": "p", "repository": "https://example.org/p", "extensions": {"my-harness": decl}}).to_string(),
        );
        assert_eq!(
            declared(read(Standard::Plugin, &plugin))
                .repository
                .as_deref(),
            Some("https://example.org/p")
        );
        // A server's standard repository is its code's, not where its
        // declaration changes.
        let server = write(
            &dir,
            "server.json",
            &json!({"name": "s", "repository": {"url": "https://example.org/code"}, "_meta": {"my-harness": decl}}).to_string(),
        );
        assert_eq!(declared(read(Standard::Mcp, &server)).repository, None);
    }

    #[test]
    fn a_declaration_of_no_contract_or_another_is_refused() {
        let dir = TempDir::new().unwrap();
        for (file, contract, why) in [
            ("a.md", "", "names no contract"),
            ("b.md", "  my-harness.contract: \"2\"\n", "contract 2"),
            ("c.md", "  my-harness.contract: one\n", "contract \"one\""),
        ] {
            let text = format!("---\nname: s\nmetadata:\n{contract}  my-harness.scope: turn\n  my-harness.job: j\n---\n");
            let err = read(Standard::Skill, &write(&dir, file, &text)).unwrap_err();
            assert!(err.contains(why), "{file}: {err}");
        }
    }

    #[test]
    fn a_unit_without_a_declaration_is_undeclared_with_its_description() {
        let dir = TempDir::new().unwrap();
        let skill = write(&dir, "SKILL.md", "---\nname: s\ndescription: writes\n---\n");
        let plugin = write(&dir, "plugin.json", r#"{"name":"p","description":"tools"}"#);
        let undeclared = |d: &str| {
            Ok(Slot::Undeclared {
                description: Some(d.into()),
            })
        };
        assert_eq!(read(Standard::Skill, &skill), undeclared("writes"));
        assert_eq!(read(Standard::Plugin, &plugin), undeclared("tools"));
    }

    #[test]
    fn a_malformed_declaration_says_why() {
        let dir = TempDir::new().unwrap();
        let cases = [
            ("a.md", "---\nname: s\nmetadata:\n  my-harness.scope: 3\n---\n", "not a string"),
            ("b.md", "---\nname: s\nmetadata:\n  my-harness.contract: \"1\"\n  my-harness.scope: always\n  my-harness.job: j\n---\n", "declaration"),
            ("c.md", "---\nname: s\nmetadata:\n  my-harness.contract: \"1\"\n  my-harness.job: j\n---\n", "scope"),
            ("d.md", "---\nname: s\nmetadata:\n  my-harness.contract: \"1\"\n  my-harness.scope: turn\n  my-harness.job: j\n  my-harness.state.local: nowhere\n---\n", "not a config key"),
        ];
        for (file, text, why) in cases {
            let err = read(Standard::Skill, &write(&dir, file, text)).unwrap_err();
            assert!(err.contains(why), "{file}: {err}");
        }
        let p = write(&dir, "p.json", r#"{"extensions":{"my-harness":"turn"}}"#);
        assert!(read(Standard::Plugin, &p)
            .unwrap_err()
            .contains("not an object"));
        let p = write(&dir, "q.json", "not json");
        assert!(read(Standard::Mcp, &p).unwrap_err().contains("not JSON"));
    }

    #[test]
    fn a_rules_pointer_names_one_section_of_a_file_beside_the_unit() {
        let dir = TempDir::new().unwrap();
        let unit = write(
            &dir,
            "SKILL.md",
            "# s\n\n## Output rule 0 — announce, always\n\nfirst\n\n````\n```\n## not a heading\n````\n\n### inner\n\nkept\n\n  ## Next ##\n\nlater\n",
        );
        assert_eq!(
            slug("Output rule 0 — announce, always"),
            "output-rule-0--announce-always"
        );
        assert_eq!(
            rules(&unit, "SKILL.md#output-rule-0--announce-always").unwrap(),
            "## Output rule 0 — announce, always\n\nfirst\n\n````\n```\n## not a heading\n````\n\n### inner\n\nkept"
        );
        assert_eq!(
            rules(&unit, "SKILL.md#next").unwrap(),
            "  ## Next ##\n\nlater"
        );
        assert_eq!(rules(&unit, "SKILL.md#inner").unwrap(), "### inner\n\nkept");
        for (pointer, why) in [
            ("SKILL.md#not-a-heading", "no heading"),
            ("SKILL.md#nowhere", "no heading"),
            ("gone.md#next", "cannot read"),
            ("../SKILL.md#next", "unit's dir"),
            ("/etc/passwd#x", "unit's dir"),
            ("SKILL.md", "<file>#<anchor>"),
            ("#next", "<file>#<anchor>"),
        ] {
            let err = rules(&unit, pointer).unwrap_err();
            assert!(err.contains(why), "{pointer}: {err}");
        }
    }

    #[test]
    fn an_unreadable_or_relative_unit_says_why() {
        let dir = TempDir::new().unwrap();
        let gone = dir.path().join("gone.md");
        assert!(read(Standard::Skill, &gone)
            .unwrap_err()
            .contains("cannot read"));
        assert!(read(Standard::Skill, dir.path())
            .unwrap_err()
            .contains("not a regular file"));
        let big = write(&dir, "big.json", &" ".repeat(MAX_MANIFEST as usize + 1));
        assert!(read(Standard::Plugin, &big)
            .unwrap_err()
            .contains("larger than"));
        let relative = Path::new("SKILL.md");
        assert!(read(Standard::Skill, relative)
            .unwrap_err()
            .contains("not an absolute path"));
    }
}
