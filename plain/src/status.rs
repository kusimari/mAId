//! Status: what plain placed, and whether each item is still there,
//! registered and runnable. `cmd_status` reports mAId's own placement;
//! whole-status checks every package's install record (and, for one
//! release, old part list) against the agent's own listing or config,
//! any package's, not only mAId's. The ghost's view of the same items is
//! `ghost status`. See project.md "My harness" (the install record).
//!
//! Reads only: files under `$HOME` and the agents' listing commands.

use crate::claude_config::ConfigDir;
use crate::place::{listed_marketplaces, listed_plugins};
use crate::record::What;
use crate::shared::{claude_env, find_on, Agent};
use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One entry as a reader sees it: `kind` is a string, so a kind this
/// reader does not know is one entry it cannot check, not a refused file.
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct Entry {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub skipped: Option<String>,
    #[serde(default)]
    pub value: Option<Value>,
}

/// A package's install record, or an old part list, as read.
#[derive(Debug)]
pub struct Part {
    pub name: String,
    harnesses: BTreeMap<String, Vec<Entry>>,
    /// An install record, not an old part list.
    manifest: bool,
}

/// A file this reader would not take, and why.
#[derive(Debug, PartialEq)]
pub struct Refused {
    pub file: String,
    pub why: String,
}

/// Every non-dot `*.json` in each root, roots in order, files in name
/// order. A missing root has none. An old part list is not read when an
/// install record has its name.
pub fn read_parts(roots: &[PathBuf]) -> Result<(Vec<Part>, Vec<Refused>)> {
    let mut files: Vec<PathBuf> = vec![];
    for dir in roots {
        let mut here: Vec<PathBuf> = match fs::read_dir(dir) {
            Ok(rd) => rd
                .map(|e| e.map(|e| e.path()))
                .collect::<io::Result<_>>()
                .with_context(|| format!("cannot read {}", dir.display()))?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => vec![],
            Err(e) => return Err(e).with_context(|| format!("cannot read {}", dir.display())),
        };
        here.retain(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            !name.starts_with('.') && p.extension() == Some(OsStr::new("json"))
        });
        here.sort();
        files.extend(here);
    }
    let read: Vec<(String, Result<Part, String>)> = files
        .iter()
        .map(|p| {
            (
                p.file_name().unwrap_or_default().to_string_lossy().into(),
                read_part(p),
            )
        })
        .collect();
    let manifests: BTreeSet<String> = read
        .iter()
        .filter_map(|(_, p)| {
            p.as_ref()
                .ok()
                .filter(|p| p.manifest)
                .map(|p| p.name.clone())
        })
        .collect();
    let (mut parts, mut refused) = (vec![], vec![]);
    for (file, part) in read {
        match part {
            Ok(p) if !p.manifest && manifests.contains(&p.name) => {}
            Ok(p) => parts.push(p),
            Err(why) => refused.push(Refused { file, why }),
        }
    }
    Ok((parts, refused))
}

fn read_part(path: &Path) -> Result<Part, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("cannot read: {e}"))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| format!("not JSON: {e}"))?;
    match &v["format"] {
        // 1.0 and 1e0 too: a reader with one number type cannot tell them
        // from 1.
        Value::Number(n) if n.as_f64() == Some(1.0) => {}
        Value::Null => return Err("no format".into()),
        other => return Err(format!("format {other} is not one this reader knows")),
    }
    let manifest = !v["package"].is_null();
    let (name, key, what) = match manifest {
        true => ("package", "agents", "an install record"),
        false => ("part", "harnesses", "a part list"),
    };
    let name: String =
        serde_json::from_value(v[name].clone()).map_err(|e| format!("not {what}: {name}: {e}"))?;
    let harnesses: BTreeMap<String, Vec<Entry>> =
        serde_json::from_value(v[key].clone()).map_err(|e| format!("not {what}: {key}: {e}"))?;
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    if name != stem {
        return Err(format!(
            "{} {name:?} is not the file's name",
            if manifest { "package" } else { "part" }
        ));
    }
    Ok(Part {
        name,
        harnesses,
        manifest,
    })
}

/// What the harness says about one entry.
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    Present,
    /// Listed, and the listing says the user turned it off.
    Disabled,
    Missing,
    /// Registered, but its command is not there to run.
    NotRunnable(String),
    Skipped(String),
    NotCheckable(String),
    Unreadable(String),
}

impl Found {
    fn label(&self) -> &'static str {
        match self {
            Found::Present => "present",
            Found::Disabled => "present (disabled)",
            Found::Missing => "missing",
            Found::NotRunnable(_) => "not runnable",
            Found::Skipped(_) => "skipped",
            Found::NotCheckable(_) => "not checkable",
            Found::Unreadable(_) => "unreadable",
        }
    }

    fn why(&self) -> Option<&str> {
        match self {
            Found::Skipped(w)
            | Found::NotCheckable(w)
            | Found::Unreadable(w)
            | Found::NotRunnable(w) => Some(w),
            Found::Present | Found::Disabled | Found::Missing => None,
        }
    }
}

/// One checked entry.
#[derive(Debug)]
pub struct Line {
    pub harness: String,
    pub part: String,
    pub entry: Entry,
    pub found: Found,
}

/// The whole report: every checked entry, then every refused file.
#[derive(Debug)]
pub struct Whole {
    /// The folders read: install records, then the old part lists.
    pub dirs: Vec<PathBuf>,
    /// How many lists were read, whatever the agent selected.
    pub parts: usize,
    pub lines: Vec<Line>,
    pub refused: Vec<Refused>,
}

impl Whole {
    /// Whether anything placed is missing or could not be read.
    pub fn failed(&self) -> bool {
        !self.refused.is_empty()
            || self.lines.iter().any(|l| {
                matches!(
                    l.found,
                    Found::Missing | Found::Unreadable(_) | Found::NotRunnable(_)
                )
            })
    }
}

impl fmt::Display for Whole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.lines.is_empty() && self.refused.is_empty() {
            return match self.parts {
                0 => {
                    let dirs: Vec<String> =
                        self.dirs.iter().map(|d| d.display().to_string()).collect();
                    writeln!(f, "no install records in {}", dirs.join(" or "))
                }
                _ => writeln!(f, "no entries to check"),
            };
        }
        let (mut harness, mut part) = (None, None);
        for l in &self.lines {
            if harness != Some(&l.harness) {
                writeln!(f, "{}", l.harness)?;
                (harness, part) = (Some(&l.harness), None);
            }
            if part != Some(&l.part) {
                writeln!(f, "  {}", l.part)?;
                part = Some(&l.part);
            }
            let e = &l.entry;
            write!(f, "    {:<14} {} {}", l.found.label(), e.kind, e.name)?;
            if let Some(s) = &e.scope {
                write!(f, " [{s}]")?;
            }
            match l.found.why() {
                Some(why) => writeln!(f, " ({why})")?,
                None => writeln!(f)?,
            }
        }
        for r in &self.refused {
            writeln!(f, "refused  {}: {}", r.file, r.why)?;
        }
        Ok(())
    }
}

/// Every package's install record (and old part list), checked against
/// the harnesses on this PATH.
pub fn cmd_whole(
    lists: &crate::record::Lists,
    home: &Path,
    claude: &ConfigDir,
    agent: Option<Agent>,
) -> Result<u8> {
    let roots = [lists.records.clone(), lists.old.clone()];
    let whole = check(&roots, home, claude, std::env::var_os("PATH"), agent)?;
    print!("{whole}");
    Ok(u8::from(whole.failed()))
}

/// Check every install record and old part list in `dirs` against the
/// agents under `home`,
/// Claude's in `claude`, running their CLIs from `path`. `agent` scopes to
/// one harness.
pub fn check(
    dirs: &[PathBuf],
    home: &Path,
    claude: &ConfigDir,
    path: Option<OsString>,
    agent: Option<Agent>,
) -> Result<Whole> {
    let (parts, refused) = read_parts(dirs)?;
    let probe = Probe {
        home: home.to_path_buf(),
        claude: claude.clone(),
        path,
        seen: RefCell::default(),
    };
    let mut keys: BTreeSet<&str> = parts
        .iter()
        .flat_map(|p| p.harnesses.keys().map(String::as_str))
        .collect();
    // Known harnesses first, in mAId's order; any other key after.
    let mut order: Vec<&str> = Agent::ALL
        .iter()
        .map(|a| a.cli())
        .filter(|k| keys.remove(k))
        .collect();
    order.extend(keys);
    if let Some(a) = agent {
        order.retain(|k| *k == a.cli());
    }
    let mut lines = vec![];
    for harness in order {
        for part in &parts {
            for entry in part.harnesses.get(harness).into_iter().flatten() {
                lines.push(Line {
                    harness: harness.into(),
                    part: part.name.clone(),
                    entry: entry.clone(),
                    found: probe.found(harness, entry),
                });
            }
        }
    }
    Ok(Whole {
        dirs: dirs.to_vec(),
        parts: parts.len(),
        lines,
        refused,
    })
}

/// The same, with Claude's config under `home` and `CLAUDE_CONFIG_DIR`
/// unset, so a test does not depend on the machine.
pub fn check_in(
    dirs: &[PathBuf],
    home: &Path,
    path: Option<OsString>,
    agent: Option<Agent>,
) -> Result<Whole> {
    check(dirs, home, &ConfigDir::new(home, None), path, agent)
}

/// Where the names of one kind on one harness are read from.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Source {
    /// `plugin list --json`; for claude, only the entries at this scope.
    Plugins(Agent, Option<String>),
    /// `plugin marketplace list --json`.
    Marketplaces(Agent),
    /// `codex mcp list --json`.
    CodexMcp,
    /// Each `<name>/SKILL.md` in this dir.
    Skills(PathBuf),
    /// The frontmatter `name` of each `*.md` in this dir.
    MarkdownAgents(PathBuf),
    /// The `name` (or file name) of each `*.json` in this dir.
    JsonAgents(PathBuf),
    /// The keys of one object in a JSON file.
    Keys(PathBuf, &'static str),
    /// A settings file, read per entry (rule 14).
    Settings(PathBuf),
    /// The `mcpServers` keys of every kiro agent file with this name.
    KiroAgentMcp(String),
}

const KIRO_AGENTS: &str = ".kiro/agents";

/// How a kind is read on a harness, or why it cannot be.
/// Every path is absolute: Claude's from `claude`, the rest under `home`.
fn source(
    agent: Agent,
    kind: What,
    scope: Option<&str>,
    home: &Path,
    claude: &ConfigDir,
) -> Result<Source, String> {
    use Agent::{Claude, Codex, Kiro};
    use What::{Marketplace, Mcp, Plugin, Setting, Skill};
    let one_place = |src: Source| match scope {
        None => Ok(src),
        Some(s) => Err(format!("{} has no scope {s:?} to read", agent.cli())),
    };
    match (agent, kind) {
        // claude lists project and local plugins for the dir it runs in.
        (Claude, Plugin) if matches!(scope, None | Some("user")) => {
            Ok(Source::Plugins(Claude, Some("user".into())))
        }
        (Claude, Plugin) => Err(format!(
            "claude lists {} plugins per directory",
            scope.unwrap_or_default()
        )),
        // `claude mcp list` and `get` start each server to health-check it.
        (Claude, Mcp) if matches!(scope, None | Some("user")) => {
            Ok(Source::Keys(claude.global(), "mcpServers"))
        }
        (Kiro, Mcp) => Ok(match scope {
            Some(s) => Source::KiroAgentMcp(s.into()),
            None => Source::Keys(home.join(".kiro/settings/mcp.json"), "mcpServers"),
        }),
        (Claude, Skill) => one_place(Source::Skills(claude.dir().join("skills"))),
        (_, Skill) => one_place(Source::Skills(home.join(agent.user_skills_dir()))),
        (Claude, Marketplace) | (Codex, Marketplace) => one_place(Source::Marketplaces(agent)),
        (Codex, Plugin) => one_place(Source::Plugins(Codex, None)),
        (Codex, Mcp) => one_place(Source::CodexMcp),
        (Claude, What::Agent) => one_place(Source::MarkdownAgents(claude.dir().join("agents"))),
        (Kiro, What::Agent) => one_place(Source::JsonAgents(home.join(KIRO_AGENTS))),
        (Claude, Setting) => one_place(Source::Settings(claude.settings())),
        (Kiro, Setting) => one_place(Source::Settings(home.join(".kiro/settings/cli.json"))),
        _ => Err(match scope {
            Some(s) if agent == Claude && kind == Mcp => {
                format!("claude {s} MCP servers are not read")
            }
            Some(s) if kind == Mcp => format!("{} has no scope {s:?} to read", agent.cli()),
            _ => format!("no way to read a {} on {}", kind_name(kind), agent.cli()),
        }),
    }
}

fn kind_name(kind: What) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Reads each source at most once per run.
struct Probe {
    home: PathBuf,
    claude: ConfigDir,
    path: Option<OsString>,
    seen: RefCell<HashMap<Source, Result<Listing, Found>>>,
}

impl Probe {
    fn found(&self, harness: &str, entry: &Entry) -> Found {
        if let Some(why) = &entry.skipped {
            return Found::Skipped(why.clone());
        }
        let Some(agent) = Agent::ALL.iter().copied().find(|a| a.cli() == harness) else {
            return Found::NotCheckable(format!("{harness:?} is not a harness this reader knows"));
        };
        let Ok(kind) = What::parse(&entry.kind) else {
            return Found::NotCheckable(format!(
                "{:?} is not a kind this reader knows",
                entry.kind
            ));
        };
        let src = match source(
            agent,
            kind,
            entry.scope.as_deref(),
            &self.home,
            &self.claude,
        ) {
            Ok(src) => src,
            Err(why) => return Found::NotCheckable(why),
        };
        if let Source::Settings(file) = &src {
            return self.setting(file, entry);
        }
        let servers = match &src {
            Source::Keys(file, "mcpServers") => Some(file.clone()),
            _ => None,
        };
        let mut seen = self.seen.borrow_mut();
        let names = seen.entry(src).or_insert_with_key(|src| self.names(src));
        let listing = match names {
            Ok(listing) => listing,
            Err(found) => return found.clone(),
        };
        match (listing.names.get(&entry.name), &listing.unread) {
            (Some(Seen::On), _) => match servers.and_then(|f| self.not_runnable(&f, &entry.name)) {
                Some(why) => Found::NotRunnable(why),
                None => Found::Present,
            },
            (Some(Seen::Off), _) => Found::Disabled,
            (Some(Seen::Broken(why)), _) | (None, Some(why)) => Found::Unreadable(why.clone()),
            (None, None) => Found::Missing,
        }
    }

    /// Why the MCP server `name` registered in `file` cannot run: its
    /// command is a path that is not an executable file, or a name this
    /// PATH does not have. None when it can, or says no command.
    fn not_runnable(&self, file: &Path, name: &str) -> Option<String> {
        let v: Value = serde_json::from_str(&fs::read_to_string(file).ok()?).ok()?;
        let command = v["mcpServers"][name]["command"].as_str()?;
        let runs = match command.contains('/') {
            true => {
                use std::os::unix::fs::PermissionsExt;
                fs::metadata(command)
                    .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            }
            false => find_on(self.path.as_deref(), command).is_some(),
        };
        (!runs).then(|| format!("its command {command} is not there to run"))
    }

    fn names(&self, src: &Source) -> Result<Listing, Found> {
        let unreadable = |e: String| Found::Unreadable(e);
        match src {
            Source::Plugins(agent, scope) => {
                let v = self.cli(*agent, &["plugin", "list", "--json"])?;
                let (entries, id) = listed_plugins(*agent, &v)
                    .ok_or_else(|| unreadable("plugin list: unexpected shape".into()))?;
                let at_scope = entries
                    .iter()
                    .filter(|e| scope.as_ref().is_none_or(|s| e["scope"] == **s));
                Ok(by_key(at_scope, id))
            }
            Source::Marketplaces(agent) => {
                let v = self.cli(*agent, &["plugin", "marketplace", "list", "--json"])?;
                let (entries, _) = listed_marketplaces(*agent, &v)
                    .ok_or_else(|| unreadable("marketplace list: unexpected shape".into()))?;
                Ok(by_key(entries, "name"))
            }
            Source::CodexMcp => {
                let v = self.cli(Agent::Codex, &["mcp", "list", "--json"])?;
                let entries = v
                    .as_array()
                    .ok_or_else(|| unreadable("mcp list: unexpected shape".into()))?;
                Ok(by_key(entries, "name"))
            }
            Source::Skills(dir) => Ok(on(self
                .dir(dir)?
                .into_iter()
                .filter(|p| p.join("SKILL.md").is_file())
                .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into())))),
            Source::MarkdownAgents(dir) => Ok(self
                .files(dir, "md")?
                .iter()
                .filter_map(|p| match frontmatter_name(p) {
                    Ok(name) => name.map(|n| (n, Seen::On)),
                    Err(why) => Some((
                        stem(p),
                        Seen::Broken(format!("{}/{}: {why}", self.shown(dir), file(p))),
                    )),
                })
                .collect()),
            Source::JsonAgents(dir) => Ok(self
                .json_agents(dir)?
                .into_iter()
                .map(|(name, v)| match v {
                    Ok(_) => (name, Seen::On),
                    Err(why) => (name, Seen::Broken(why)),
                })
                .collect()),
            Source::Keys(file, object) => {
                let shown = self.shown(file);
                let v = match fs::read_to_string(file) {
                    Ok(text) => serde_json::from_str::<Value>(&text)
                        .map_err(|e| unreadable(format!("{shown}: not JSON: {e}")))?,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Listing::default()),
                    Err(e) => return Err(unreadable(format!("{shown}: {e}"))),
                };
                let Value::Object(top) = &v else {
                    return Err(unreadable(format!("{shown}: not a JSON object")));
                };
                members(top, object).map_err(|why| unreadable(format!("{shown}: {why}")))
            }
            Source::Settings(_) => unreachable!("a setting is read per entry, by setting()"),
            // Every agent file of that name counts, and one that cannot be
            // read could hold any server.
            Source::KiroAgentMcp(agent) => {
                let mut listing = Listing::default();
                for (name, parsed) in self.json_agents(&self.home.join(KIRO_AGENTS))? {
                    if name != *agent {
                        continue;
                    }
                    let servers = parsed.and_then(|o| {
                        members(&o, "mcpServers")
                            .map_err(|why| format!("~/{KIRO_AGENTS}/{name}: {why}"))
                    });
                    match servers {
                        Ok(servers) => listing.extend(servers.names),
                        Err(why) => listing.unread = Some(why),
                    }
                }
                Ok(listing)
            }
        }
    }

    /// A setting: `name` is a top-level key, or, with no such key, a dotted
    /// path to a nested one (kiro's own keys hold dots). With a `value`, the
    /// setting, or one element of a list setting, must be that string.
    fn setting(&self, file: &Path, entry: &Entry) -> Found {
        // Strings only: readers in other languages disagree on numbers.
        let want = match &entry.value {
            None => None,
            Some(Value::String(s)) => Some(s.as_str()),
            Some(_) => return Found::NotCheckable("a setting's value is not a string".into()),
        };
        let shown = self.shown(file);
        let v = match fs::read_to_string(file) {
            Ok(text) => match serde_json::from_str::<Value>(&text) {
                Ok(v) => v,
                Err(e) => return Found::Unreadable(format!("{shown}: not JSON: {e}")),
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Found::Missing,
            Err(e) => return Found::Unreadable(format!("{shown}: {e}")),
        };
        let Some(top) = v.as_object() else {
            return Found::Unreadable(format!("{shown}: not an object"));
        };
        let at = top.get(&entry.name).or_else(|| {
            entry
                .name
                .split('.')
                .try_fold(&v, |v, key| v.as_object().and_then(|o| o.get(key)))
        });
        let is_want = |v: &Value| v.as_str() == want;
        let found = match (at, want) {
            (None, _) => false,
            (Some(_), None) => true,
            (Some(Value::Array(list)), Some(_)) => list.iter().any(is_want),
            (Some(v), Some(_)) => is_want(v),
        };
        if found {
            Found::Present
        } else {
            Found::Missing
        }
    }

    /// A harness's listing, as JSON. A CLI not on the PATH cannot be asked.
    fn cli(&self, agent: Agent, args: &[&str]) -> Result<Value, Found> {
        let cli = agent.cli();
        let at = find_on(self.path.as_deref(), cli)
            .ok_or_else(|| Found::NotCheckable(format!("{cli} not on PATH")))?;
        let mut cmd = Command::new(at);
        cmd.args(args).env("HOME", &self.home);
        if agent == Agent::Claude {
            claude_env(&mut cmd, &self.claude);
        }
        if let Some(p) = &self.path {
            cmd.env("PATH", p);
        }
        let said = |why: String| Found::Unreadable(format!("{cli} {}: {why}", args.join(" ")));
        let out = cmd.output().map_err(|e| said(e.to_string()))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            let first = err.lines().map(str::trim).find(|l| !l.is_empty());
            return Err(said(first.unwrap_or("failed").into()));
        }
        serde_json::from_slice(&out.stdout).map_err(|e| said(format!("not JSON: {e}")))
    }

    /// A path as messages show it: `~/` for one under HOME.
    fn shown(&self, p: &Path) -> String {
        match p.strip_prefix(&self.home) {
            Ok(rel) => format!("~/{}", rel.display()),
            Err(_) => p.display().to_string(),
        }
    }

    /// The entries of a dir; none when it is not there.
    fn dir(&self, dir: &Path) -> Result<Vec<PathBuf>, Found> {
        let unreadable = |e: io::Error| Found::Unreadable(format!("{}: {e}", self.shown(dir)));
        match fs::read_dir(dir) {
            Ok(rd) => rd
                .map(|e| e.map(|e| e.path()))
                .collect::<io::Result<_>>()
                .map_err(unreadable),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(vec![]),
            Err(e) => Err(unreadable(e)),
        }
    }

    fn files(&self, dir: &Path, ext: &str) -> Result<Vec<PathBuf>, Found> {
        let mut files = self.dir(dir)?;
        files.retain(|p| p.extension() == Some(OsStr::new(ext)) && p.is_file());
        Ok(files)
    }

    /// Each `*.json` agent in a dir, by its `name` or, with none, its
    /// stem; one that is not a JSON object goes by its stem.
    fn json_agents(&self, dir: &Path) -> Result<Vec<(String, Parsed)>, Found> {
        let agent = |p: PathBuf| {
            let parsed = fs::read_to_string(&p)
                .map_err(|e| e.to_string())
                .and_then(|t| {
                    serde_json::from_str::<Value>(&t).map_err(|e| format!("not JSON: {e}"))
                })
                .and_then(|v| match v {
                    Value::Object(o) => Ok(o),
                    _ => Err("not a JSON object".into()),
                })
                .map_err(|why| format!("{}/{}: {why}", self.shown(dir), file(&p)));
            let name = match &parsed {
                Ok(o) => o.get("name").and_then(Value::as_str).map(str::to_string),
                Err(_) => None,
            };
            (name.unwrap_or_else(|| stem(&p)), parsed)
        };
        Ok(self.files(dir, "json")?.into_iter().map(agent).collect())
    }
}

/// An agent file's JSON, or why it would not parse.
type Parsed = Result<serde_json::Map<String, Value>, String>;

/// What a source lists, by name. Where several items or files give one
/// name, the best state wins, so the result never depends on their order.
#[derive(Default)]
struct Listing {
    names: BTreeMap<String, Seen>,
    /// A file that could name more could not be read: a name not listed is
    /// unreadable, not missing.
    unread: Option<String>,
}

impl Extend<(String, Seen)> for Listing {
    fn extend<I: IntoIterator<Item = (String, Seen)>>(&mut self, items: I) {
        for (name, seen) in items {
            let held = self.names.entry(name).or_insert_with(|| seen.clone());
            if seen < *held {
                *held = seen;
            }
        }
    }
}

impl FromIterator<(String, Seen)> for Listing {
    fn from_iter<I: IntoIterator<Item = (String, Seen)>>(items: I) -> Listing {
        let mut listing = Listing::default();
        listing.extend(items);
        listing
    }
}

/// Best first: the order `Ord` derives from.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Seen {
    On,
    Off,
    /// A file that should name one could not be read; keyed by its stem.
    Broken(String),
}

fn on(names: impl IntoIterator<Item = String>) -> Listing {
    names.into_iter().map(|n| (n, Seen::On)).collect()
}

/// The keys of `object[key]`: none when it is absent or null.
fn members(object: &serde_json::Map<String, Value>, key: &str) -> Result<Listing, String> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(Listing::default()),
        Some(Value::Object(o)) => Ok(on(o.keys().cloned())),
        Some(_) => Err(format!("{key} is not an object")),
    }
}

/// Entries named by `key`; one with `"enabled": false` is off.
fn by_key<'a>(entries: impl IntoIterator<Item = &'a Value>, key: &str) -> Listing {
    entries
        .into_iter()
        .filter_map(|e| {
            let seen = match e["enabled"] == false {
                true => Seen::Off,
                false => Seen::On,
            };
            e[key].as_str().map(|n| (n.to_string(), seen))
        })
        .collect()
}

fn stem(p: &Path) -> String {
    p.file_stem().unwrap_or_default().to_string_lossy().into()
}

fn file(p: &Path) -> String {
    p.file_name().unwrap_or_default().to_string_lossy().into()
}

/// A claude agent's name, read as text so any reader can do the same (see
/// project.md "My harness"). `Ok(None)`: the file names nothing.
fn frontmatter_name(path: &Path) -> Result<Option<String>, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    if text.starts_with('\u{feff}') {
        return Err("starts with a byte-order mark".into());
    }
    let lines: Vec<&str> = text
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    let Some((&"---", rest)) = lines.split_first() else {
        return Ok(None);
    };
    let Some(end) = rest.iter().position(|l| *l == "---") else {
        return Ok(None);
    };
    Ok(rest[..end]
        .iter()
        .find_map(|l| l.strip_prefix("name:"))
        .map(|v| {
            let v = v
                .split(" #")
                .next()
                .unwrap_or_default()
                .trim_matches([' ', '\t']);
            ['"', '\'']
                .iter()
                .find_map(|q| v.strip_prefix(*q).and_then(|v| v.strip_suffix(*q)))
                .unwrap_or(v)
                .to_string()
        }))
}

pub fn cmd_status(target: &impl crate::place::Deploy, agent: Option<Agent>) -> Result<u8> {
    for report in target.status(agent)? {
        println!("{:<28} {}", report.label, report.state.describe());
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    /// A parts dir, a HOME and a bin dir, all temporary.
    struct World {
        parts: TempDir,
        home: TempDir,
        bin: TempDir,
    }

    impl World {
        fn new() -> World {
            World {
                parts: TempDir::new().unwrap(),
                home: TempDir::new().unwrap(),
                bin: TempDir::new().unwrap(),
            }
        }

        fn part(&self, file: &str, text: &str) {
            fs::write(self.parts.path().join(file), text).unwrap();
        }

        /// One part, `p`, with these entries on one harness.
        fn entries(&self, harness: &str, entries: &str) {
            self.part(
                "p.json",
                &format!(r#"{{"format":1,"part":"p","harnesses":{{"{harness}":[{entries}]}}}}"#),
            );
        }

        fn file(&self, rel: &str, text: &str) {
            let at = self.home.path().join(rel);
            fs::create_dir_all(at.parent().unwrap()).unwrap();
            fs::write(at, text).unwrap();
        }

        /// A CLI that prints `out` for exactly `args`, and fails otherwise.
        /// Each call leaves a mark, so a test can see it was not run.
        fn stub(&self, cli: &str, cases: &[(&str, &str)]) {
            let mut body = format!(
                ": > '{}/{cli}.called'\ncase \"$*\" in\n",
                self.bin.path().display()
            );
            for (args, out) in cases {
                body += &format!("  '{args}') printf '%s' '{out}' ;;\n");
            }
            body += "  *) echo \"stub: $*\" >&2; exit 3 ;;\nesac\n";
            self.script(cli, &body);
        }

        /// Write an executable script, and wait until it can be run: a
        /// child another test thread forks while the file is open for
        /// writing holds it busy (ETXTBSY) until that child execs.
        fn script(&self, cli: &str, body: &str) {
            let at = self.bin.path().join(cli);
            let text = format!("#!/bin/sh\n[ \"$1\" = --ready ] && exit 0\n{body}");
            fs::write(&at, text).unwrap();
            fs::set_permissions(&at, fs::Permissions::from_mode(0o755)).unwrap();
            while let Err(e) = Command::new(&at).arg("--ready").status() {
                assert_eq!(e.raw_os_error(), Some(26), "{e}");
                std::thread::yield_now();
            }
        }

        fn called(&self, cli: &str) -> bool {
            self.bin.path().join(format!("{cli}.called")).exists()
        }

        fn run(&self, agent: Option<Agent>) -> Whole {
            let path = Some(self.bin.path().as_os_str().to_owned());
            check(
                &[self.parts.path().to_path_buf()],
                self.home.path(),
                &ConfigDir::new(self.home.path(), None),
                path,
                agent,
            )
            .unwrap()
        }

        fn found(&self) -> Vec<(String, Found)> {
            self.run(None)
                .lines
                .into_iter()
                .map(|l| (l.entry.name, l.found))
                .collect()
        }
    }

    fn present_and_missing(w: &World, harness: &str, kind: &str) {
        w.entries(
            harness,
            &format!(r#"{{"kind":"{kind}","name":"here"}},{{"kind":"{kind}","name":"gone"}}"#),
        );
        assert_eq!(
            w.found(),
            vec![
                ("here".into(), Found::Present),
                ("gone".into(), Found::Missing)
            ],
            "{harness} {kind}"
        );
    }

    fn not_checkable(found: &[(String, Found)]) -> bool {
        !found.is_empty()
            && found
                .iter()
                .all(|(_, f)| matches!(f, Found::NotCheckable(_)))
    }

    // ── reader ───────────────────────────────────────────────────

    #[test]
    fn every_parts_list_is_read_and_nothing_else() {
        let w = World::new();
        w.part("a.json", r#"{"format":1,"part":"a","harnesses":{}}"#);
        w.part(
            "b.json",
            r#"{"format":1,"part":"b","harnesses":{},"later":true}"#,
        );
        w.part(".c.json", "half a list");
        w.part("notes.txt", "not a list");
        let (parts, refused) = read_parts(&[w.parts.path().to_path_buf()]).unwrap();
        let names: Vec<_> = parts.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["a", "b"]);
        assert_eq!(refused, vec![]);
    }

    #[test]
    fn a_file_that_is_not_a_format_1_list_is_refused_with_why() {
        let w = World::new();
        w.part("bad.json", "{");
        w.part("later.json", r#"{"format":2,"shape":"new"}"#);
        w.part("none.json", r#"{"part":"x","harnesses":{}}"#);
        w.part("shape.json", r#"{"format":1,"part":"x"}"#);
        w.part(
            "wrong.json",
            r#"{"format":1,"part":"other","harnesses":{}}"#,
        );
        w.part(
            "xentry.json",
            r#"{"format":1,"part":"xentry","harnesses":{"agy":[{"name":"s"}]}}"#,
        );
        let (parts, refused) = read_parts(&[w.parts.path().to_path_buf()]).unwrap();
        assert!(parts.is_empty());
        let why: Vec<_> = refused.iter().map(|r| (r.file.as_str(), &r.why)).collect();
        assert!(
            why[0].0 == "bad.json" && why[0].1.starts_with("not JSON"),
            "{why:?}"
        );
        assert!(
            why[1].0 == "later.json" && why[1].1.contains("format 2"),
            "{why:?}"
        );
        assert!(
            why[2].0 == "none.json" && why[2].1 == "no format",
            "{why:?}"
        );
        assert!(
            why[3].0 == "shape.json" && why[3].1.contains("harnesses"),
            "{why:?}"
        );
        assert!(
            why[4].0 == "wrong.json" && why[4].1.contains("file's name"),
            "{why:?}"
        );
        assert!(
            why[5].0 == "xentry.json" && why[5].1.contains("kind"),
            "{why:?}"
        );
    }

    #[test]
    fn no_parts_dir_is_no_parts() {
        let w = World::new();
        let whole = check(
            &[w.parts.path().join("none")],
            w.home.path(),
            &ConfigDir::new(w.home.path(), None),
            None,
            None,
        )
        .unwrap();
        assert!(!whole.failed());
        assert!(whole.to_string().starts_with("no install records in"));
    }

    #[test]
    fn an_unknown_kind_or_harness_is_not_checkable_not_refused() {
        let w = World::new();
        w.part(
            "p.json",
            r#"{"format":1,"part":"p","harnesses":{
                "claude":[{"kind":"hook","name":"h"}],
                "other-cli":[{"kind":"skill","name":"s"}]}}"#,
        );
        let whole = w.run(None);
        assert!(whole.refused.is_empty());
        let found: Vec<_> = whole
            .lines
            .iter()
            .map(|l| (l.harness.as_str(), &l.found))
            .collect();
        assert!(matches!(found[0], ("claude", Found::NotCheckable(w)) if w.contains("hook")));
        assert!(
            matches!(found[1], ("other-cli", Found::NotCheckable(w)) if w.contains("other-cli"))
        );
        assert!(!whole.failed());
    }

    // ── each (harness, kind) ─────────────────────────────────────

    #[test]
    fn claude_plugins_at_user_scope_and_no_other() {
        let w = World::new();
        w.stub(
            "claude",
            &[(
                "plugin list --json",
                r#"[{"id":"here","scope":"user"},{"id":"gone","scope":"project"},{"id":"proj","scope":"project"}]"#,
            )],
        );
        present_and_missing(&w, "claude", "plugin");
        w.entries(
            "claude",
            r#"{"kind":"plugin","name":"proj","scope":"project"}"#,
        );
        assert!(not_checkable(&w.found()));
    }

    #[test]
    fn claude_and_codex_marketplaces_from_their_listings() {
        let w = World::new();
        w.stub(
            "claude",
            &[("plugin marketplace list --json", r#"[{"name":"here"}]"#)],
        );
        present_and_missing(&w, "claude", "marketplace");
        w.stub(
            "codex",
            &[(
                "plugin marketplace list --json",
                r#"{"marketplaces":[{"name":"here"}]}"#,
            )],
        );
        present_and_missing(&w, "codex", "marketplace");
    }

    #[test]
    fn codex_plugins_and_mcp_servers_from_its_listings() {
        let w = World::new();
        w.stub(
            "codex",
            &[
                (
                    "plugin list --json",
                    r#"{"installed":[{"pluginId":"here"}]}"#,
                ),
                ("mcp list --json", r#"[{"name":"here","enabled":true}]"#),
            ],
        );
        present_and_missing(&w, "codex", "plugin");
        present_and_missing(&w, "codex", "mcp");
    }

    #[test]
    fn skills_from_each_harness_dir() {
        for (harness, dir) in [
            ("claude", ".claude/skills"),
            ("codex", ".codex/skills"),
            ("kiro-cli", ".kiro/skills"),
            ("agy", ".gemini/config/skills"),
        ] {
            let w = World::new();
            w.file(&format!("{dir}/here/SKILL.md"), "---\nname: here\n---\n");
            fs::create_dir_all(w.home.path().join(dir).join("gone")).unwrap();
            present_and_missing(&w, harness, "skill");
        }
    }

    #[test]
    fn claude_mcp_servers_and_settings_from_its_config() {
        let w = World::new();
        w.file(".claude.json", r#"{"gone":1,"mcpServers":{"here":{}}}"#);
        present_and_missing(&w, "claude", "mcp");
        w.file(".claude/settings.json", r#"{"here":"x","env":{"gone":1}}"#);
        present_and_missing(&w, "claude", "setting");
    }

    #[test]
    fn an_mcp_server_whose_command_is_gone_is_not_runnable() {
        let w = World::new();
        let tool = w.home.path().join("tool");
        fs::write(&tool, "").unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o755)).unwrap();
        let gone = w.home.path().join("gone");
        w.file(
            ".claude.json",
            &format!(
                r#"{{"mcpServers":{{"runs":{{"command":"{}"}},"gone":{{"command":"{}"}},"named":{{"command":"no-such-tool"}}}}}}"#,
                tool.display(),
                gone.display()
            ),
        );
        w.entries(
            "claude",
            r#"{"kind":"mcp","name":"runs"},{"kind":"mcp","name":"gone"},{"kind":"mcp","name":"named"}"#,
        );
        let found = w.found();
        assert_eq!(found[0], ("runs".into(), Found::Present));
        assert!(
            matches!(&found[1].1, Found::NotRunnable(w) if w.contains("gone")),
            "{found:?}"
        );
        assert!(matches!(&found[2].1, Found::NotRunnable(_)), "{found:?}");
    }

    #[test]
    fn a_dotted_setting_reads_a_nested_key_and_a_list_its_element() {
        let w = World::new();
        w.file(
            ".claude/settings.json",
            r#"{"permissions":{"defaultMode":"auto","additionalDirectories":["/a","/b"]}}"#,
        );
        w.entries(
            "claude",
            r#"{"kind":"setting","name":"permissions.defaultMode"},
               {"kind":"setting","name":"permissions.gone"},
               {"kind":"setting","name":"permissions.defaultMode.x"},
               {"kind":"setting","name":"permissions.additionalDirectories","value":"/b"},
               {"kind":"setting","name":"permissions.additionalDirectories","value":"/c","adopted":true}"#,
        );
        let found: Vec<Found> = w.found().into_iter().map(|(_, f)| f).collect();
        use Found::{Missing, Present};
        assert_eq!(found, [Present, Missing, Missing, Present, Missing]);
    }

    #[test]
    fn a_literal_key_with_dots_wins_over_a_nested_path() {
        let w = World::new();
        w.file(
            ".kiro/settings/cli.json",
            r#"{"chat.defaultAgent":"a","chat":{"other":1,"defaultAgent":["b"]}}"#,
        );
        w.entries(
            "kiro-cli",
            r#"{"kind":"setting","name":"chat.defaultAgent","value":"a"},{"kind":"setting","name":"chat.other"}"#,
        );
        let found: Vec<Found> = w.found().into_iter().map(|(_, f)| f).collect();
        assert_eq!(found, [Found::Present, Found::Present]);
        w.file(".kiro/settings/cli.json", "[]");
        assert!(matches!(w.found()[0].1, Found::Unreadable(_)));
    }

    #[test]
    fn claude_agents_by_their_frontmatter_name() {
        let w = World::new();
        w.file(
            ".claude/agents/a.md",
            "---\nname: here\ndescription: d\n---\nbody\n",
        );
        w.file(".claude/agents/other.md", "---\nname: other\n---\n");
        present_and_missing(&w, "claude", "agent");
    }

    #[test]
    fn kiro_agents_by_their_json_name() {
        let w = World::new();
        w.file(".kiro/agents/a.json", r#"{"name":"here"}"#);
        w.file(".kiro/agents/b.json", r#"{"name":"other"}"#);
        present_and_missing(&w, "kiro-cli", "agent");
    }

    #[test]
    fn an_agent_file_that_does_not_parse_is_unreadable_not_missing() {
        let w = World::new();
        w.file(".kiro/agents/broken.json", "{ half");
        w.file(".kiro/agents/fine.json", r#"{"name":"fine"}"#);
        w.file(
            ".claude/agents/broken.md",
            "\u{feff}---\nname: broken\n---\n",
        );
        w.part(
            "p.json",
            r#"{"format":1,"part":"p","harnesses":{
                "kiro-cli":[{"kind":"agent","name":"broken"},{"kind":"agent","name":"fine"},
                            {"kind":"mcp","name":"m","scope":"broken"}],
                "claude":[{"kind":"agent","name":"broken"}]}}"#,
        );
        let found = w.found();
        let unreadable =
            |i: usize, file: &str| matches!(&found[i].1, Found::Unreadable(w) if w.contains(file));
        assert!(unreadable(0, "broken.md"), "{found:?}");
        assert!(
            unreadable(1, "broken.json") && unreadable(3, "broken.json"),
            "{found:?}"
        );
        assert_eq!(found[2].1, Found::Present);
    }

    #[test]
    fn a_listed_entry_turned_off_is_present_and_disabled_and_passes() {
        let w = World::new();
        w.stub(
            "codex",
            &[
                (
                    "plugin list --json",
                    r#"{"installed":[{"pluginId":"p","enabled":false}]}"#,
                ),
                (
                    "mcp list --json",
                    r#"[{"name":"m","enabled":false},{"name":"n","enabled":true}]"#,
                ),
            ],
        );
        w.entries(
            "codex",
            r#"{"kind":"plugin","name":"p"},{"kind":"mcp","name":"m"},{"kind":"mcp","name":"n"}"#,
        );
        let found: Vec<_> = w.found().into_iter().map(|(_, f)| f).collect();
        assert_eq!(
            found,
            vec![Found::Disabled, Found::Disabled, Found::Present]
        );
        let whole = w.run(None);
        assert!(!whole.failed());
        assert!(
            whole
                .to_string()
                .contains("    present (disabled) plugin p\n"),
            "{whole}"
        );
    }

    #[test]
    fn a_kiro_agent_with_no_name_goes_by_its_file_name() {
        let w = World::new();
        w.file(".kiro/agents/here.json", r#"{"mcpServers":{"m":{}}}"#);
        present_and_missing(&w, "kiro-cli", "agent");
        w.entries("kiro-cli", r#"{"kind":"mcp","name":"m","scope":"here"}"#);
        assert_eq!(w.found(), vec![("m".into(), Found::Present)]);
    }

    #[test]
    fn kiro_mcp_servers_per_agent_or_global() {
        let w = World::new();
        w.file(
            ".kiro/agents/x.json",
            r#"{"name":"mine","mcpServers":{"here":{}}}"#,
        );
        w.file(".kiro/settings/mcp.json", r#"{"mcpServers":{"global":{}}}"#);
        w.entries(
            "kiro-cli",
            r#"{"kind":"mcp","name":"here","scope":"mine"},
               {"kind":"mcp","name":"global","scope":"mine"},
               {"kind":"mcp","name":"here","scope":"nobody"},
               {"kind":"mcp","name":"global"},
               {"kind":"mcp","name":"here"}"#,
        );
        let found: Vec<_> = w.found().into_iter().map(|(_, f)| f).collect();
        use Found::*;
        assert_eq!(found, vec![Present, Missing, Missing, Present, Missing]);
    }

    #[test]
    fn kiro_settings_from_its_cli_settings() {
        let w = World::new();
        w.file(".kiro/settings/cli.json", r#"{"here":"x"}"#);
        present_and_missing(&w, "kiro-cli", "setting");
    }

    #[test]
    fn kinds_a_harness_has_no_listing_for_are_not_checkable() {
        let w = World::new();
        for (harness, kind) in [
            ("codex", "agent"),
            ("codex", "setting"),
            ("kiro-cli", "plugin"),
            ("kiro-cli", "marketplace"),
            ("agy", "mcp"),
            ("agy", "agent"),
            ("agy", "plugin"),
            ("agy", "marketplace"),
            ("agy", "setting"),
        ] {
            w.entries(harness, &format!(r#"{{"kind":"{kind}","name":"x"}}"#));
            assert!(not_checkable(&w.found()), "{harness} {kind}");
        }
    }

    #[test]
    fn a_scope_where_a_harness_has_one_place_is_not_checkable() {
        let w = World::new();
        w.file(".codex/skills/x/SKILL.md", "");
        w.entries(
            "codex",
            r#"{"kind":"skill","name":"x","scope":"somewhere"}"#,
        );
        assert!(not_checkable(&w.found()));
        w.file(".claude.json", r#"{"mcpServers":{"x":{}}}"#);
        w.entries("claude", r#"{"kind":"mcp","name":"x","scope":"project"}"#);
        let found = w.found();
        assert!(not_checkable(&found));
        assert_eq!(
            found[0].1,
            Found::NotCheckable("claude project MCP servers are not read".into())
        );
    }

    // ── skipped, no CLI, unreadable ──────────────────────────────

    #[test]
    fn a_skipped_entry_is_reported_and_not_checked() {
        let w = World::new();
        w.stub("claude", &[]);
        w.entries(
            "claude",
            r#"{"kind":"plugin","name":"x","skipped":"no network"}"#,
        );
        assert_eq!(
            w.found(),
            vec![("x".into(), Found::Skipped("no network".into()))]
        );
        assert!(!w.called("claude"));
    }

    #[test]
    fn with_no_cli_its_listings_are_not_checkable_and_its_files_are_read() {
        let w = World::new();
        w.file(".claude/skills/s/SKILL.md", "");
        w.entries(
            "claude",
            r#"{"kind":"plugin","name":"p"},{"kind":"marketplace","name":"m"},{"kind":"skill","name":"s"}"#,
        );
        let found: Vec<_> = w.found().into_iter().map(|(_, f)| f).collect();
        let no_cli = Found::NotCheckable("claude not on PATH".into());
        assert_eq!(found, vec![no_cli.clone(), no_cli, Found::Present]);
    }

    #[test]
    fn a_listing_that_fails_or_is_not_json_is_unreadable() {
        let w = World::new();
        w.stub("codex", &[("mcp list --json", "not json")]);
        w.entries(
            "codex",
            r#"{"kind":"mcp","name":"m"},{"kind":"plugin","name":"p"}"#,
        );
        let found = w.found();
        assert!(
            matches!(&found[0].1, Found::Unreadable(w) if w.contains("not JSON")),
            "{found:?}"
        );
        assert!(
            matches!(&found[1].1, Found::Unreadable(w) if w.contains("stub: plugin list --json")),
            "{found:?}"
        );
        w.file(".claude.json", "{");
        w.entries("claude", r#"{"kind":"mcp","name":"m"}"#);
        assert!(matches!(&w.found()[0].1, Found::Unreadable(_)));
        w.file(".claude.json", r#"{"mcpServers":[]}"#);
        assert!(matches!(&w.found()[0].1, Found::Unreadable(w) if w.contains("not an object")));
    }

    #[test]
    fn a_dir_that_cannot_be_read_is_unreadable() {
        let w = World::new();
        w.file(".kiro/skills", "a file where the dir should be");
        w.file(".kiro/agents", "");
        w.entries(
            "kiro-cli",
            r#"{"kind":"skill","name":"s"},{"kind":"agent","name":"a"},{"kind":"mcp","name":"m","scope":"a"}"#,
        );
        let found = w.found();
        assert!(
            found.iter().all(|(_, f)| matches!(f, Found::Unreadable(_))),
            "{found:?}"
        );
    }

    #[test]
    fn a_failed_listing_names_its_first_words() {
        let w = World::new();
        w.script("codex", "echo >&2\necho '  config is broken' >&2\nexit 1\n");
        w.entries("codex", r#"{"kind":"mcp","name":"m"}"#);
        let found = w.found();
        assert!(
            matches!(&found[0].1, Found::Unreadable(w) if w.ends_with(": config is broken")),
            "{found:?}"
        );
    }

    #[test]
    fn each_listing_runs_once_per_status() {
        let w = World::new();
        let log = w.bin.path().join("calls");
        w.script(
            "codex",
            &format!("echo x >> '{}'\nprintf '%s' '[]'\n", log.display()),
        );
        w.entries(
            "codex",
            r#"{"kind":"mcp","name":"a"},{"kind":"mcp","name":"b"}"#,
        );
        w.run(None);
        assert_eq!(fs::read_to_string(log).unwrap().lines().count(), 1);
    }

    // ── report and exit ──────────────────────────────────────────

    #[test]
    fn only_missing_unreadable_or_refused_fails() {
        let w = World::new();
        w.file(".kiro/skills/s/SKILL.md", "");
        w.entries(
            "kiro-cli",
            r#"{"kind":"skill","name":"s"},{"kind":"plugin","name":"p"},{"kind":"skill","name":"k","skipped":"why"}"#,
        );
        assert!(!w.run(None).failed());
        w.entries("kiro-cli", r#"{"kind":"skill","name":"gone"}"#);
        assert!(w.run(None).failed());
        w.entries("codex", r#"{"kind":"plugin","name":"p"}"#);
        w.stub("codex", &[]);
        assert!(w.run(None).failed());
        w.entries("agy", "");
        w.part("bad.json", "{");
        assert!(w.run(None).failed());
    }

    #[test]
    fn the_report_groups_by_harness_then_part() {
        let w = World::new();
        w.file(".kiro/skills/s/SKILL.md", "");
        w.part(
            "a.json",
            r#"{"format":1,"part":"a","harnesses":{
                "kiro-cli":[{"kind":"skill","name":"s"}],
                "claude":[{"kind":"mcp","name":"m","skipped":"no Chrome"}]}}"#,
        );
        w.part(
            "b.json",
            r#"{"format":1,"part":"b","harnesses":{"kiro-cli":[{"kind":"mcp","name":"m","scope":"mine"}]}}"#,
        );
        w.part("c.json", "[]");
        let text = w.run(None).to_string();
        let want = "\
claude
  a
    skipped        mcp m (no Chrome)
kiro-cli
  a
    present        skill s
  b
    missing        mcp m [mine]
refused  c.json: no format
";
        assert_eq!(text, want);
    }

    #[test]
    fn an_agent_selects_one_harness() {
        let w = World::new();
        w.part(
            "a.json",
            r#"{"format":1,"part":"a","harnesses":{
                "kiro-cli":[{"kind":"skill","name":"s"}],
                "claude":[{"kind":"skill","name":"s"}],
                "other":[{"kind":"skill","name":"s"}]}}"#,
        );
        let whole = w.run(Some(Agent::Kiro));
        let harnesses: Vec<_> = whole.lines.iter().map(|l| l.harness.as_str()).collect();
        assert_eq!(harnesses, ["kiro-cli"]);
        let none = w.run(Some(Agent::Agy)).to_string();
        assert_eq!(none, "no entries to check\n");
    }
}
