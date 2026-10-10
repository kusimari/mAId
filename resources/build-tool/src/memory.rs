//! Memory stores: which stores are configured, where Claude's automatic
//! memory goes, and harvesting each harness's own memory into a store.
//! See project.md "My harness", Memory.
//!
//! The config is one file per part under the memory config dir; readers
//! take the union. mAId writes only its own file.

use crate::part::{Item, What, PART};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const FORMAT: u32 = 1;
pub const SETTING: &str = "autoMemoryDirectory";
/// What Claude loads of `MEMORY.md` at session start.
const INDEX_LINES: usize = 200;
const INDEX_BYTES: usize = 25 * 1024;

// ─────────────────────────────────────────────────────────────────
// Config — every part's stores, and the one default.
// ─────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
struct PartConfig {
    format: u32,
    part: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    default: Option<String>,
    #[serde(default)]
    stores: BTreeMap<String, PathBuf>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Store {
    pub name: String,
    pub path: PathBuf,
    pub part: String,
}

impl Store {
    pub fn memory_dir(&self) -> PathBuf {
        self.path.join("memory")
    }
}

/// The union of every part's file. A name two parts configure, or a second
/// default, is a problem: that name or the default is left unusable rather
/// than guessed.
#[derive(Debug, Default)]
pub struct Config {
    pub stores: Vec<Store>,
    pub default: Option<String>,
    pub problems: Vec<String>,
}

impl Config {
    pub fn read(dir: &Path) -> Result<Config> {
        let mut files = Vec::new();
        match fs::read_dir(dir) {
            Ok(entries) => {
                for entry in entries {
                    let path = entry?.path();
                    let name = path.file_name().unwrap_or_default().to_string_lossy();
                    if !name.starts_with('.') && name.ends_with(".json") && path.is_file() {
                        files.push(path);
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e).with_context(|| format!("cannot read {}", dir.display())),
        }
        files.sort();

        let mut config = Config::default();
        let mut by_name: BTreeMap<String, Vec<Store>> = BTreeMap::new();
        let mut defaults = Vec::new();
        for path in files {
            let file = path.file_name().unwrap_or_default().to_string_lossy();
            let parsed: Result<PartConfig, _> = fs::read_to_string(&path)
                .map_err(anyhow::Error::from)
                .and_then(|t| serde_json::from_str(&t).map_err(Into::into));
            let pc = match parsed {
                Ok(pc) if pc.format == FORMAT => pc,
                Ok(pc) => {
                    config
                        .problems
                        .push(format!("{file} is format {}; ignored", pc.format));
                    continue;
                }
                Err(e) => {
                    config
                        .problems
                        .push(format!("{file} is not a memory config: {e}"));
                    continue;
                }
            };
            for (name, store_path) in pc.stores {
                by_name.entry(name.clone()).or_default().push(Store {
                    name,
                    path: store_path,
                    part: pc.part.clone(),
                });
            }
            if let Some(d) = pc.default {
                defaults.push((pc.part, d));
            }
        }
        for (name, mut stores) in by_name {
            match stores.len() {
                1 => config.stores.push(stores.remove(0)),
                _ => config.problems.push(format!(
                    "store {name} is configured by more than one part ({}); unusable",
                    stores
                        .iter()
                        .map(|s| s.part.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )),
            }
        }
        match defaults.as_slice() {
            [] => {}
            [(_, name)] if config.store(name).is_some() => config.default = Some(name.clone()),
            [(part, name)] => config.problems.push(format!(
                "default {name} (set by {part}) is not a usable store; no default"
            )),
            many => config.problems.push(format!(
                "more than one part sets a default ({}); no default",
                many.iter()
                    .map(|(p, n)| format!("{p}: {n}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
        Ok(config)
    }

    pub fn store(&self, name: &str) -> Option<&Store> {
        self.stores.iter().find(|s| s.name == name)
    }

    pub fn default_store(&self) -> Option<&Store> {
        self.default.as_deref().and_then(|n| self.store(n))
    }

    /// The store a write goes to: the named one, else the default. Never a
    /// store that is not configured or whose path is gone.
    pub fn resolve(&self, name: Option<&str>) -> Result<&Store> {
        let store = match name {
            Some(n) => self
                .store(n)
                .ok_or_else(|| anyhow!("no memory store named {n} is configured"))?,
            None => self
                .default_store()
                .ok_or_else(|| anyhow!("no default memory store is configured"))?,
        };
        if !store.path.is_dir() {
            bail!(
                "memory store {} is missing (configured at {})",
                store.name,
                store.path.display()
            );
        }
        Ok(store)
    }
}

/// mAId's own config file.
pub fn own_file(dir: &Path) -> PathBuf {
    dir.join(format!("{PART}.json"))
}

fn read_own(dir: &Path) -> Result<PartConfig> {
    let at = own_file(dir);
    let pc: PartConfig = match fs::read_to_string(&at) {
        Ok(text) => serde_json::from_str(&text)
            .with_context(|| format!("{} is not a memory config", at.display()))?,
        Err(e) if e.kind() == io::ErrorKind::NotFound => PartConfig {
            format: FORMAT,
            part: PART.into(),
            ..PartConfig::default()
        },
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", at.display())),
    };
    if pc.format != FORMAT || pc.part != PART {
        bail!(
            "{} is format {} of part {:?}; refusing to rewrite it",
            at.display(),
            pc.format,
            pc.part
        );
    }
    Ok(pc)
}

fn write_own(dir: &Path, pc: &PartConfig) -> Result<()> {
    let at = own_file(dir);
    if pc.stores.is_empty() && pc.default.is_none() {
        return match fs::remove_file(&at) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        };
    }
    write_atomic(
        &at,
        format!("{}\n", serde_json::to_string_pretty(pc)?).as_bytes(),
    )
}

/// Write through a temp file and rename, so a reader never sees half a
/// file. A link is written through to the file it names, and that file
/// keeps its mode (a new one gets 0644), so a file another tool manages
/// stays where and what it was.
fn write_atomic(at: &Path, bytes: &[u8]) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let target = match fs::symlink_metadata(at) {
        Ok(m) if m.file_type().is_symlink() => {
            fs::canonicalize(at).with_context(|| format!("{} links to nothing", at.display()))?
        }
        _ => at.to_path_buf(),
    };
    let mode = fs::metadata(&target)
        .map(|m| m.permissions())
        .unwrap_or_else(|_| fs::Permissions::from_mode(0o644));
    let dir = target.parent().context("a file needs a parent dir")?;
    fs::create_dir_all(dir)?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)
        .with_context(|| format!("cannot write beside {}", target.display()))?;
    tmp.write_all(bytes)?;
    fs::set_permissions(tmp.path(), mode)?;
    tmp.persist(&target)?;
    Ok(())
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        && !name.starts_with('.')
}

/// Add or update one of mAId's stores; `default` also names it the default,
/// which only one part may do.
pub fn add_store(dir: &Path, name: &str, path: &Path, default: bool) -> Result<String> {
    if !valid_name(name) {
        bail!("store name {name:?}: use letters, digits, '-', '_' or '.'");
    }
    let path = fs::canonicalize(path)
        .ok()
        .filter(|p| p.is_dir())
        .ok_or_else(|| {
            anyhow!(
                "{} is not a directory; clone the store first",
                path.display()
            )
        })?;
    let all = Config::read(dir)?;
    if let Some(other) = all.store(name).filter(|s| s.part != PART) {
        bail!("store {name} is configured by part {}", other.part);
    }
    let mut pc = read_own(dir)?;
    let before = pc.default.clone();
    if default {
        if let Some(part) = other_default(dir)? {
            bail!("part {part} already names the default memory store");
        }
        pc.default = Some(name.into());
    }
    let moved = pc.stores.get(name).is_some_and(|p| *p != path);
    pc.stores.insert(name.into(), path.clone());
    write_own(dir, &pc)?;
    let is_default = pc.default.as_deref() == Some(name);
    let mut out = format!(
        "store    {name} -> {}{}",
        path.display(),
        if is_default { " (default)" } else { "" }
    );
    if pc.default != before || (is_default && moved) {
        out.push_str(APPLY);
    }
    Ok(out)
}

/// Said when the default store changes; the store verbs never write
/// claude's settings themselves.
const APPLY: &str = "\nrun `just install` to apply to claude";

/// The part other than mAId whose file names a default, if any.
fn other_default(dir: &Path) -> Result<Option<String>> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(None);
    };
    let mut found = None;
    for entry in entries {
        let path = entry?.path();
        let file = path.file_name().unwrap_or_default().to_string_lossy();
        if file.starts_with('.') || !file.ends_with(".json") || path == own_file(dir) {
            continue;
        }
        if let Ok(pc) = fs::read_to_string(&path)
            .map_err(anyhow::Error::from)
            .and_then(|t| serde_json::from_str::<PartConfig>(&t).map_err(Into::into))
        {
            if pc.format == FORMAT && pc.part != PART && pc.default.is_some() {
                found = Some(pc.part);
            }
        }
    }
    Ok(found)
}

pub fn remove_store(dir: &Path, name: &str) -> Result<String> {
    let mut pc = read_own(dir)?;
    if pc.stores.remove(name).is_none() {
        return match Config::read(dir)?.store(name) {
            Some(s) => Err(anyhow!("store {name} is configured by part {}", s.part)),
            None => Err(anyhow!("no memory store named {name} is configured")),
        };
    }
    let was_default = pc.default.as_deref() == Some(name);
    if was_default {
        pc.default = None;
    }
    write_own(dir, &pc)?;
    Ok(format!(
        "removed  store {name}{}",
        if was_default {
            format!(" (was the default){APPLY}")
        } else {
            String::new()
        }
    ))
}

// ─────────────────────────────────────────────────────────────────
// Claude Code — `autoMemoryDirectory` in user settings.
//
// One value: mAId sets it only when mAId's own file names the default and
// the key is empty or already mAId's, and removes its own value when it no
// longer has a default to point at.
// ─────────────────────────────────────────────────────────────────

/// Why install did not place the setting. Fixed phrases, no paths.
pub const NO_CLI: &str = "claude not on PATH";
pub const NO_DEFAULT: &str = "mAId names no default memory store";
pub const DEFAULT_MISSING: &str = "default memory store missing";
pub const OTHER_VALUE: &str = "not mAId's";

pub fn claude_settings(home: &Path) -> PathBuf {
    home.join(".claude/settings.json")
}

/// mAId owns the `autoMemoryDirectory` value only when it is exactly the
/// value its part list records having set (rule 2: a part owns what it
/// wrote). A hand-set value is another's, even one naming a mAId store.
#[derive(Debug, Default)]
pub struct Ownership {
    pub recorded: Option<String>,
}

impl Ownership {
    fn owns(&self, raw: &str) -> bool {
        self.recorded.as_deref() == Some(raw)
    }
}

/// An absent key, `null` and `""` all count as empty.
fn current_value(map: &serde_json::Map<String, serde_json::Value>) -> Option<&serde_json::Value> {
    map.get(SETTING)
        .filter(|v| !v.is_null() && v.as_str() != Some(""))
}

/// What mAId would set: its own default store's memory dir, or why none.
fn wanted(config: &Config) -> std::result::Result<PathBuf, &'static str> {
    let store = config
        .default_store()
        .filter(|s| s.part == PART)
        .ok_or(NO_DEFAULT)?;
    match store.path.is_dir() {
        true => Ok(store.memory_dir()),
        false => Err(DEFAULT_MISSING),
    }
}

/// The outcome of one pass over the setting: the line to print, and the
/// part-list entry (none once mAId holds nothing there).
pub struct Outcome {
    pub line: String,
    pub item: Option<Item>,
}

fn read_settings(at: &Path) -> Result<serde_json::Map<String, serde_json::Value>> {
    match fs::read_to_string(at) {
        Ok(text) if text.trim().is_empty() => Ok(Default::default()),
        Ok(text) => match serde_json::from_str(&text)
            .with_context(|| format!("{} is not JSON", at.display()))?
        {
            serde_json::Value::Object(map) => Ok(map),
            _ => bail!("{} is not a JSON object", at.display()),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Default::default()),
        Err(e) => Err(e).with_context(|| format!("cannot read {}", at.display())),
    }
}

fn is_ours(own: &Ownership, value: &serde_json::Value) -> bool {
    value.as_str().is_some_and(|raw| own.owns(raw))
}

/// Install: point Claude's automatic memory at mAId's default store, or
/// take mAId's value away when there is none to point at.
pub fn install_claude(
    home: &Path,
    config: &Config,
    own: &Ownership,
    cli_present: bool,
    dry_run: bool,
) -> Result<Outcome> {
    let skipped = |why: &str| Item::skipped(What::Setting, SETTING, why);
    let tag = if dry_run { "(dry-run) " } else { "" };
    let at = claude_settings(home);
    if !cli_present {
        // Nothing is written, so an unreadable file is no reason to fail. A
        // value mAId set before stays recorded as placed, so a later install
        // can still tell it is mAId's.
        let map = read_settings(&at).unwrap_or_default();
        let item = match current_value(&map) {
            None => skipped(NO_CLI),
            Some(v) if is_ours(own, v) => Item {
                value: v.as_str().map(str::to_string),
                ..Item::placed(What::Setting, SETTING)
            },
            Some(_) => skipped(OTHER_VALUE),
        };
        return Ok(Outcome {
            line: format!("{tag}skip          claude {SETTING} ({NO_CLI})"),
            item: Some(item),
        });
    }
    let mut map = read_settings(&at)?;
    let current = current_value(&map).cloned();
    let ours = current.as_ref().is_none_or(|v| is_ours(own, v));
    let want = wanted(config);
    let (line, item) = match (ours, &want) {
        // Another's value is the reason, whatever else also applies.
        (false, _) => (
            format!("{tag}skip          claude {SETTING} ({OTHER_VALUE})"),
            Some(skipped(OTHER_VALUE)),
        ),
        (true, Ok(dir)) => {
            let value = serde_json::Value::String(dir.display().to_string());
            if current.as_ref() != Some(&value) {
                map.insert(SETTING.into(), value);
                if !dry_run {
                    write_settings(&at, &map)?;
                }
            }
            (
                format!("{tag}done          claude {SETTING} -> {}", dir.display()),
                Some(Item {
                    value: Some(dir.display().to_string()),
                    ..Item::placed(What::Setting, SETTING)
                }),
            )
        }
        (true, Err(why)) => {
            let line = match current.is_some() {
                true => {
                    map.remove(SETTING);
                    if !dry_run {
                        write_settings(&at, &map)?;
                    }
                    format!("{tag}removed       claude {SETTING} ({why})")
                }
                false => format!("{tag}skip          claude {SETTING} ({why})"),
            };
            (line, Some(skipped(why)))
        }
    };
    Ok(Outcome { line, item })
}

/// Uninstall: remove the value only while it is still mAId's.
pub fn uninstall_claude(home: &Path, own: &Ownership, dry_run: bool) -> Result<Outcome> {
    let at = claude_settings(home);
    let tag = if dry_run { "(dry-run) " } else { "" };
    let mut map = read_settings(&at)?;
    let line = match current_value(&map) {
        Some(v) if is_ours(own, v) => {
            map.remove(SETTING);
            if !dry_run {
                write_settings(&at, &map)?;
            }
            format!("{tag}removed       claude {SETTING}")
        }
        Some(_) => format!("{tag}kept          claude {SETTING} (not mAId's)"),
        None => format!("{tag}done          claude {SETTING} (not set)"),
    };
    Ok(Outcome { line, item: None })
}

/// Values are re-serialised: key order is kept, but a number or escape may
/// come back in another text form.
fn write_settings(at: &Path, map: &serde_json::Map<String, serde_json::Value>) -> Result<()> {
    let text = serde_json::to_string_pretty(map)?;
    write_atomic(at, format!("{text}\n").as_bytes())
}

/// Status: every store, the problems, and where Claude's memory goes.
pub fn status(home: &Path, config: &Config, own: &Ownership) -> Result<Vec<String>> {
    let mut lines: Vec<String> = config
        .stores
        .iter()
        .map(|s| {
            let mut notes = vec![s.part.clone()];
            if config.default.as_deref() == Some(s.name.as_str()) {
                notes.insert(0, "default".into());
            }
            if !s.path.is_dir() {
                notes.push("MISSING".into());
            }
            format!(
                "memory   store {} -> {} ({})",
                s.name,
                s.path.display(),
                notes.join(", ")
            )
        })
        .collect();
    if config.stores.is_empty() {
        lines.push("memory   no stores configured (just memory-store <name> <path>)".into());
    }
    lines.extend(
        config
            .problems
            .iter()
            .map(|p| format!("memory   problem: {p}")),
    );
    let map = read_settings(&claude_settings(home))?;
    lines.push(match current_value(&map) {
        None => format!("memory   claude {SETTING} not set (per-project memory)"),
        Some(v) => {
            let shown = v.as_str().map_or_else(|| v.to_string(), str::to_string);
            let whose = if is_ours(own, v) {
                "mAId's"
            } else {
                "not mAId's"
            };
            format!("memory   claude {SETTING} -> {shown} ({whose})")
        }
    });
    Ok(lines)
}

// ─────────────────────────────────────────────────────────────────
// Harvest — copy each harness's own memory into a store.
//
// Originals are only read. A file whose bytes already sit at `<stem>.md`
// or `<stem>-N.md` is skipped, else it lands at the first free name, so a
// rerun changes nothing.
// ─────────────────────────────────────────────────────────────────

const INDEX: &str = "MEMORY.md";

struct Candidate {
    from: String,
    stem: String,
    bytes: Vec<u8>,
    /// The source index's line for this file, with `{file}` where the link
    /// target goes.
    line: String,
}

#[derive(Debug, Default, PartialEq)]
pub struct Harvest {
    pub copied: Vec<(String, String)>,
    pub renamed: Vec<(String, String)>,
    pub present: Vec<String>,
    pub notes: Vec<String>,
}

impl Harvest {
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        out.extend(
            self.copied
                .iter()
                .map(|(from, to)| format!("copied   {from} -> {to}")),
        );
        out.extend(
            self.renamed.iter().map(|(from, to)| {
                format!("renamed  {from} -> {to} (name taken by another memory)")
            }),
        );
        out.extend(self.present.iter().map(|f| format!("present  {f}")));
        out.extend(self.notes.iter().cloned());
        if self.copied.is_empty() && self.renamed.is_empty() {
            out.push("nothing new to harvest".into());
        }
        out
    }
}

/// The source index's line linking `file`, with the link left as `{file}`.
fn index_line(index: &str, file: &str) -> Option<String> {
    let link = format!("]({file})");
    index
        .lines()
        .find(|l| l.contains(&link))
        .map(|l| l.replacen(&link, "]({file})", 1))
}

/// An index line from a memory's own frontmatter, for a file the source
/// index does not list.
fn generated_line(bytes: &[u8], stem: &str) -> String {
    #[derive(Deserialize, Default)]
    struct Front {
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        description: Option<String>,
    }
    let front = gray_matter::Matter::<gray_matter::engine::YAML>::new()
        .parse::<Front>(&String::from_utf8_lossy(bytes))
        .ok()
        .and_then(|p| p.data)
        .unwrap_or_default();
    // One line per memory, and the name must not close the link early.
    let flat = |t: String| t.split_whitespace().collect::<Vec<_>>().join(" ");
    let name = flat(front.name.unwrap_or_else(|| stem.to_string())).replace(['[', ']'], "");
    match front.description.map(flat) {
        Some(d) if !d.is_empty() => format!("- [{name}]({{file}}) - {d}"),
        _ => format!("- [{name}]({{file}})"),
    }
}

fn claude_candidates(home: &Path, target: &Path) -> Result<Vec<Candidate>> {
    let projects = home.join(".claude/projects");
    let target = fs::canonicalize(target).ok();
    let mut dirs: Vec<PathBuf> = match fs::read_dir(&projects) {
        Ok(e) => e
            .filter_map(|e| e.ok().map(|e| e.path().join("memory")))
            .filter(|d| d.is_dir())
            .collect(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => vec![],
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", projects.display())),
    };
    dirs.sort();
    let mut out = Vec::new();
    for dir in dirs {
        if target.is_some() && fs::canonicalize(&dir).ok() == target {
            continue;
        }
        let index = fs::read_to_string(dir.join(INDEX)).unwrap_or_default();
        let mut files: Vec<PathBuf> = fs::read_dir(&dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.is_file()
                    && p.extension().is_some_and(|x| x == "md")
                    && p.file_name().is_some_and(|n| n != INDEX)
            })
            .collect();
        files.sort();
        for path in files {
            let file = path.file_name().unwrap().to_string_lossy().to_string();
            let stem = path.file_stem().unwrap().to_string_lossy().to_string();
            let bytes = fs::read(&path)?;
            let line = index_line(&index, &file).unwrap_or_else(|| generated_line(&bytes, &stem));
            out.push(Candidate {
                from: path.display().to_string(),
                stem,
                bytes,
                line,
            });
        }
    }
    Ok(out)
}

/// codex keeps one consolidated registry rather than a file per memory, so
/// each of its two summaries becomes one reference memory.
fn codex_candidates(codex_home: &Path) -> Result<Vec<Candidate>> {
    let dir = codex_home.join("memories");
    let mut out = Vec::new();
    for (file, stem, what) in [
        ("MEMORY.md", "codex-memory", "codex's memory registry"),
        (
            "memory_summary.md",
            "codex-memory-summary",
            "codex's memory summary",
        ),
    ] {
        let path = dir.join(file);
        let body = match fs::read_to_string(&path) {
            Ok(body) => body,
            Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e).with_context(|| format!("cannot read {}", path.display())),
        };
        let bytes = format!(
            "---\nname: {stem}\ndescription: {what}, harvested from codex\nmetadata:\n  type: reference\n---\n\n{body}"
        )
        .into_bytes();
        out.push(Candidate {
            from: path.display().to_string(),
            stem: stem.into(),
            line: format!("- [{stem}]({{file}}) - {what}, harvested from codex"),
            bytes,
        });
    }
    Ok(out)
}

/// Copy every harness's native memory into `store`'s memory dir.
pub fn harvest(home: &Path, codex_home: &Path, store: &Store) -> Result<Harvest> {
    let target = store.memory_dir();
    let mut report = Harvest::default();
    if !target.is_dir() {
        fs::create_dir_all(&target)?;
        report.notes.push(format!("created  {}", target.display()));
    }
    let mut candidates = claude_candidates(home, &target)?;
    candidates.extend(codex_candidates(codex_home)?);

    let index_at = target.join(INDEX);
    let mut index = match fs::read_to_string(&index_at) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        // Never replace an index that is there but unreadable.
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", index_at.display())),
    };
    let before = index.clone();
    for c in candidates {
        let mut n = 1;
        let (file, new) = loop {
            let file = match n {
                1 => format!("{}.md", c.stem),
                _ => format!("{}-{n}.md", c.stem),
            };
            match fs::read(target.join(&file)) {
                Ok(existing) if existing == c.bytes => break (file, false),
                Ok(_) => n += 1,
                Err(e) if e.kind() == io::ErrorKind::NotFound => break (file, true),
                Err(e) => return Err(e.into()),
            }
        };
        if new {
            write_atomic(&target.join(&file), &c.bytes)?;
        }
        // Also for a file already there: a run that stopped after copying
        // it never wrote its line.
        if !index.contains(&format!("]({file})")) {
            if !index.is_empty() && !index.ends_with('\n') {
                index.push('\n');
            }
            index.push_str(&c.line.replace("{file}", &file));
            index.push('\n');
        }
        let to = target.join(&file).display().to_string();
        match (new, n) {
            (false, _) => report.present.push(c.from),
            (true, 1) => report.copied.push((c.from, to)),
            (true, _) => report.renamed.push((c.from, to)),
        }
    }
    if index != before {
        write_atomic(&index_at, index.as_bytes())?;
    }
    if index.lines().count() > INDEX_LINES || index.len() > INDEX_BYTES {
        report.notes.push(format!(
            "warning  {} is over {INDEX_LINES} lines or {} KB; Claude loads only that much at session start",
            index_at.display(),
            INDEX_BYTES / 1024
        ));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct Env {
        _tmp: TempDir,
        home: PathBuf,
        config: PathBuf,
        stores: PathBuf,
    }

    fn env() -> Env {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        for d in ["home", "config", "stores/personal", "stores/work"] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        Env {
            home: root.join("home"),
            config: root.join("config"),
            stores: fs::canonicalize(root.join("stores")).unwrap(),
            _tmp: tmp,
        }
    }

    fn other_part(e: &Env, default: Option<&str>, stores: &[(&str, &Path)]) {
        let pc = PartConfig {
            format: FORMAT,
            part: "other".into(),
            default: default.map(Into::into),
            stores: stores
                .iter()
                .map(|(n, p)| (n.to_string(), p.to_path_buf()))
                .collect(),
        };
        fs::write(
            e.config.join("other.json"),
            serde_json::to_string(&pc).unwrap(),
        )
        .unwrap();
    }

    fn cfg(e: &Env) -> Config {
        Config::read(&e.config).unwrap()
    }

    fn own(_: &Env, recorded: Option<&str>) -> Ownership {
        Ownership {
            recorded: recorded.map(Into::into),
        }
    }

    /// What install recorded for the setting, as the stage would pass it on.
    fn recorded(out: &Outcome) -> Option<String> {
        out.item.as_ref().and_then(|i| i.value.clone())
    }

    // ── config ──────────────────────────────────────────────────

    #[test]
    fn stores_from_every_part_are_read_and_dotfiles_are_not() {
        let e = env();
        add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
        other_part(&e, None, &[("work", &e.stores.join("work"))]);
        fs::write(e.config.join(".tmp.json"), "garbage").unwrap();
        let c = cfg(&e);
        let names: Vec<_> = c.stores.iter().map(|s| (&*s.name, &*s.part)).collect();
        assert_eq!(names, [("personal", PART), ("work", "other")]);
        assert_eq!(c.default.as_deref(), Some("personal"));
        assert!(c.problems.is_empty(), "{:?}", c.problems);
    }

    #[test]
    fn a_name_two_parts_configure_is_unusable() {
        let e = env();
        add_store(&e.config, "work", &e.stores.join("work"), false).unwrap();
        other_part(&e, None, &[("work", &e.stores.join("work"))]);
        let c = cfg(&e);
        assert!(c.store("work").is_none());
        assert!(
            c.problems[0].contains("more than one part"),
            "{:?}",
            c.problems
        );
        assert!(c.resolve(Some("work")).is_err());
    }

    #[test]
    fn two_defaults_leave_no_default() {
        let e = env();
        add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
        other_part(&e, Some("work"), &[("work", &e.stores.join("work"))]);
        let c = cfg(&e);
        assert_eq!(c.default, None);
        assert!(c
            .problems
            .iter()
            .any(|p| p.contains("more than one part sets a default")));
        assert!(c.resolve(None).is_err());
    }

    #[test]
    fn a_config_of_another_format_is_reported_and_not_rewritten() {
        let e = env();
        let text = r#"{"format":2,"part":"maid-harness","stores":{}}"#;
        fs::write(own_file(&e.config), text).unwrap();
        assert!(cfg(&e).problems[0].contains("format 2"));
        assert!(add_store(&e.config, "personal", &e.stores.join("personal"), false).is_err());
        assert_eq!(fs::read_to_string(own_file(&e.config)).unwrap(), text);
    }

    #[test]
    fn resolve_refuses_unknown_and_missing_stores() {
        let e = env();
        add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
        let c = cfg(&e);
        assert_eq!(c.resolve(None).unwrap().name, "personal");
        assert!(c
            .resolve(Some("nope"))
            .unwrap_err()
            .to_string()
            .contains("no memory store named nope"));
        fs::remove_dir(e.stores.join("personal")).unwrap();
        assert!(c.resolve(None).unwrap_err().to_string().contains("missing"));
    }

    // ── store verbs ─────────────────────────────────────────────

    #[test]
    fn add_refuses_a_path_that_does_not_exist() {
        let e = env();
        let err = add_store(&e.config, "x", &e.stores.join("absent"), false).unwrap_err();
        assert!(err.to_string().contains("clone the store first"));
        assert!(!own_file(&e.config).exists());
    }

    #[test]
    fn add_updates_a_store_in_place() {
        let e = env();
        add_store(&e.config, "s", &e.stores.join("personal"), false).unwrap();
        add_store(&e.config, "s", &e.stores.join("work"), false).unwrap();
        assert_eq!(cfg(&e).store("s").unwrap().path, e.stores.join("work"));
    }

    #[test]
    fn default_is_refused_while_another_part_holds_it() {
        let e = env();
        other_part(&e, Some("work"), &[("work", &e.stores.join("work"))]);
        let err = add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap_err();
        assert!(err.to_string().contains("part other"), "{err}");
        assert!(!own_file(&e.config).exists());
    }

    #[test]
    fn default_moves_between_maids_own_stores() {
        let e = env();
        add_store(&e.config, "a", &e.stores.join("personal"), true).unwrap();
        add_store(&e.config, "b", &e.stores.join("work"), true).unwrap();
        assert_eq!(cfg(&e).default.as_deref(), Some("b"));
    }

    #[test]
    fn another_parts_store_is_neither_added_over_nor_removed() {
        let e = env();
        other_part(&e, None, &[("work", &e.stores.join("work"))]);
        assert!(add_store(&e.config, "work", &e.stores.join("personal"), false).is_err());
        let err = remove_store(&e.config, "work").unwrap_err();
        assert!(err.to_string().contains("part other"));
    }

    #[test]
    fn the_verbs_say_to_install_when_the_default_changes_and_only_then() {
        let e = env();
        let apply = "run `just install` to apply to claude";
        assert!(!add_store(&e.config, "w", &e.stores.join("work"), false)
            .unwrap()
            .contains(apply));
        assert!(add_store(&e.config, "p", &e.stores.join("personal"), true)
            .unwrap()
            .contains(apply));
        assert!(!add_store(&e.config, "p", &e.stores.join("personal"), true)
            .unwrap()
            .contains(apply));
        assert!(add_store(&e.config, "p", &e.stores.join("work"), false)
            .unwrap()
            .contains(apply));
        assert!(!remove_store(&e.config, "w").unwrap().contains(apply));
        assert!(remove_store(&e.config, "p").unwrap().contains(apply));
    }

    #[test]
    fn removing_the_default_clears_it_and_the_last_store_removes_the_file() {
        let e = env();
        add_store(&e.config, "a", &e.stores.join("personal"), true).unwrap();
        let line = remove_store(&e.config, "a").unwrap();
        assert!(line.contains("was the default"));
        assert!(!own_file(&e.config).exists());
    }

    // ── claude setting ──────────────────────────────────────────

    fn settings(e: &Env) -> String {
        fs::read_to_string(claude_settings(&e.home)).unwrap()
    }

    fn seed_settings(e: &Env, text: &str) {
        fs::create_dir_all(e.home.join(".claude")).unwrap();
        fs::write(claude_settings(&e.home), text).unwrap();
    }

    #[test]
    fn install_sets_an_empty_setting_and_keeps_other_keys_in_order() {
        let e = env();
        add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
        seed_settings(&e, r#"{"zeta": 1, "alpha": {"b": 2, "a": 1}}"#);
        let out = install_claude(&e.home, &cfg(&e), &own(&e, None), true, false).unwrap();
        let want = e.stores.join("personal/memory").display().to_string();
        let item = out.item.unwrap();
        assert_eq!(
            (item.kind, item.skipped, item.value),
            (What::Setting, None, Some(want))
        );
        let text = settings(&e);
        let (z, a, m) = (
            text.find("zeta").unwrap(),
            text.find("alpha").unwrap(),
            text.find(SETTING).unwrap(),
        );
        assert!(z < a && a < m, "{text}");
        assert!(text.find("\"b\"").unwrap() < text.find("\"a\"").unwrap());
        assert!(text.contains(&e.stores.join("personal/memory").display().to_string()));
    }

    #[test]
    fn install_repoints_its_own_value_when_the_default_moves() {
        let e = env();
        add_store(&e.config, "a", &e.stores.join("personal"), true).unwrap();
        let first = install_claude(&e.home, &cfg(&e), &own(&e, None), true, false).unwrap();
        add_store(&e.config, "b", &e.stores.join("work"), true).unwrap();
        let rec = recorded(&first);
        install_claude(&e.home, &cfg(&e), &own(&e, rec.as_deref()), true, false).unwrap();
        assert!(settings(&e).contains(&e.stores.join("work/memory").display().to_string()));
    }

    #[test]
    fn install_leaves_another_value_and_records_why() {
        let e = env();
        add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
        let text = r#"{"autoMemoryDirectory": "~/elsewhere"}"#;
        seed_settings(&e, text);
        let out = install_claude(&e.home, &cfg(&e), &own(&e, None), true, false).unwrap();
        assert_eq!(out.item.unwrap().skipped.as_deref(), Some(OTHER_VALUE));
        assert_eq!(settings(&e), text);
    }

    #[test]
    fn install_removes_its_value_when_maids_config_names_no_default() {
        let e = env();
        add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
        seed_settings(&e, r#"{"keep": true}"#);
        let first = install_claude(&e.home, &cfg(&e), &own(&e, None), true, false).unwrap();
        // The default passes to another part.
        remove_store(&e.config, "personal").unwrap();
        add_store(&e.config, "personal", &e.stores.join("personal"), false).unwrap();
        other_part(&e, Some("work"), &[("work", &e.stores.join("work"))]);
        let rec = recorded(&first);
        let out = install_claude(&e.home, &cfg(&e), &own(&e, rec.as_deref()), true, false).unwrap();
        assert_eq!(out.item.unwrap().skipped.as_deref(), Some(NO_DEFAULT));
        assert!(out.line.starts_with("removed"));
        assert_eq!(settings(&e).trim(), "{\n  \"keep\": true\n}");
    }

    #[test]
    fn install_removes_its_value_when_the_default_store_is_gone() {
        let e = env();
        add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
        let first = install_claude(&e.home, &cfg(&e), &own(&e, None), true, false).unwrap();
        fs::remove_dir(e.stores.join("personal")).unwrap();
        let rec = recorded(&first);
        let out = install_claude(&e.home, &cfg(&e), &own(&e, rec.as_deref()), true, false).unwrap();
        assert_eq!(out.item.unwrap().skipped.as_deref(), Some(DEFAULT_MISSING));
        assert!(!settings(&e).contains(SETTING));
    }

    #[test]
    fn install_repoints_its_value_after_the_default_store_moves_path() {
        let e = env();
        add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
        let first = install_claude(&e.home, &cfg(&e), &own(&e, None), true, false).unwrap();
        add_store(&e.config, "personal", &e.stores.join("work"), true).unwrap();
        let rec = recorded(&first);
        let out = install_claude(&e.home, &cfg(&e), &own(&e, rec.as_deref()), true, false).unwrap();
        assert!(out.line.contains("done"), "{}", out.line);
        assert!(settings(&e).contains(&e.stores.join("work/memory").display().to_string()));
    }

    #[test]
    fn its_value_goes_after_the_default_store_is_removed() {
        let e = env();
        add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
        let first = install_claude(&e.home, &cfg(&e), &own(&e, None), true, false).unwrap();
        remove_store(&e.config, "personal").unwrap();
        let rec = recorded(&first);
        let out = install_claude(&e.home, &cfg(&e), &own(&e, rec.as_deref()), true, false).unwrap();
        assert!(out.line.starts_with("removed"), "{}", out.line);
        assert!(!settings(&e).contains(SETTING));

        // And uninstall alone also takes it away.
        add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
        let again = install_claude(&e.home, &cfg(&e), &own(&e, None), true, false).unwrap();
        remove_store(&e.config, "personal").unwrap();
        uninstall_claude(&e.home, &own(&e, recorded(&again).as_deref()), false).unwrap();
        assert!(!settings(&e).contains(SETTING));
    }

    #[test]
    fn a_name_conflict_does_not_make_maids_value_anothers() {
        let e = env();
        add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
        let first = install_claude(&e.home, &cfg(&e), &own(&e, None), true, false).unwrap();
        other_part(&e, None, &[("personal", &e.stores.join("work"))]);
        uninstall_claude(&e.home, &own(&e, recorded(&first).as_deref()), false).unwrap();
        assert!(!settings(&e).contains(SETTING));
    }

    #[test]
    fn an_empty_or_null_value_counts_as_empty() {
        for text in [
            r#"{"autoMemoryDirectory": ""}"#,
            r#"{"autoMemoryDirectory": null}"#,
        ] {
            let e = env();
            add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
            seed_settings(&e, text);
            let out = install_claude(&e.home, &cfg(&e), &own(&e, None), true, false).unwrap();
            assert!(out.line.contains("done"), "{text}: {}", out.line);
        }
    }

    #[test]
    fn a_linked_settings_file_is_written_through_and_keeps_its_mode() {
        use std::os::unix::fs::PermissionsExt;
        let e = env();
        add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
        let real = e.home.join("managed-settings.json");
        fs::write(&real, "{}").unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o644)).unwrap();
        fs::create_dir_all(e.home.join(".claude")).unwrap();
        std::os::unix::fs::symlink(&real, claude_settings(&e.home)).unwrap();
        install_claude(&e.home, &cfg(&e), &own(&e, None), true, false).unwrap();
        let link = fs::symlink_metadata(claude_settings(&e.home)).unwrap();
        assert!(link.file_type().is_symlink());
        assert!(fs::read_to_string(&real).unwrap().contains(SETTING));
        assert_eq!(
            fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o644
        );
    }

    #[test]
    fn a_value_set_before_claude_went_away_stays_maids() {
        let e = env();
        add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
        let first = install_claude(&e.home, &cfg(&e), &own(&e, None), true, false).unwrap();
        let gone = install_claude(
            &e.home,
            &cfg(&e),
            &own(&e, recorded(&first).as_deref()),
            false,
            false,
        )
        .unwrap();
        assert_eq!(recorded(&gone), recorded(&first));
        remove_store(&e.config, "personal").unwrap();
        install_claude(
            &e.home,
            &cfg(&e),
            &own(&e, recorded(&gone).as_deref()),
            true,
            false,
        )
        .unwrap();
        assert!(!settings(&e).contains(SETTING));
    }

    #[test]
    fn an_empty_key_it_never_set_is_left_alone() {
        let e = env();
        let text = r#"{"autoMemoryDirectory": ""}"#;
        seed_settings(&e, text);
        let out = install_claude(&e.home, &cfg(&e), &own(&e, None), true, false).unwrap();
        assert!(out.line.contains("skip"), "{}", out.line);
        assert_eq!(settings(&e), text);
    }

    #[test]
    fn an_unreadable_own_file_does_not_stop_install() {
        let e = env();
        fs::write(own_file(&e.config), "not json").unwrap();
        let out = install_claude(&e.home, &cfg(&e), &own(&e, None), true, false).unwrap();
        assert_eq!(out.item.unwrap().skipped.as_deref(), Some(NO_DEFAULT));
    }

    #[test]
    fn a_hand_set_value_is_not_maids_whatever_else_applies() {
        let e = env();
        let hand = r#"{"autoMemoryDirectory": "/by/hand"}"#;
        seed_settings(&e, hand);
        for cli in [true, false] {
            let out = install_claude(&e.home, &cfg(&e), &own(&e, None), cli, false).unwrap();
            assert_eq!(
                out.item.unwrap().skipped.as_deref(),
                Some(OTHER_VALUE),
                "cli {cli}"
            );
        }
        assert_eq!(settings(&e), hand);
    }

    #[test]
    fn without_claude_a_broken_settings_file_is_no_failure() {
        let e = env();
        seed_settings(&e, "not json");
        let out = install_claude(&e.home, &cfg(&e), &own(&e, None), false, false).unwrap();
        assert_eq!(out.item.unwrap().skipped.as_deref(), Some(NO_CLI));
    }

    #[test]
    fn install_without_claude_touches_nothing() {
        let e = env();
        add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
        let out = install_claude(&e.home, &cfg(&e), &own(&e, None), false, false).unwrap();
        assert_eq!(out.item.unwrap().skipped.as_deref(), Some(NO_CLI));
        assert!(!claude_settings(&e.home).exists());
    }

    #[test]
    fn install_with_no_store_writes_no_settings_file() {
        let e = env();
        let out = install_claude(&e.home, &cfg(&e), &own(&e, None), true, false).unwrap();
        assert_eq!(out.item.unwrap().skipped.as_deref(), Some(NO_DEFAULT));
        assert!(!claude_settings(&e.home).exists());
    }

    #[test]
    fn dry_run_writes_nothing() {
        let e = env();
        add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
        install_claude(&e.home, &cfg(&e), &own(&e, None), true, true).unwrap();
        assert!(!claude_settings(&e.home).exists());
    }

    #[test]
    fn uninstall_removes_only_maids_value() {
        let e = env();
        add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
        let first = install_claude(&e.home, &cfg(&e), &own(&e, None), true, false).unwrap();
        uninstall_claude(&e.home, &own(&e, recorded(&first).as_deref()), false).unwrap();
        assert!(!settings(&e).contains(SETTING));

        let other = r#"{"autoMemoryDirectory": "/somewhere/else"}"#;
        seed_settings(&e, other);
        uninstall_claude(&e.home, &own(&e, None), false).unwrap();
        assert_eq!(settings(&e), other);
    }

    #[test]
    fn a_hand_set_value_naming_a_maids_store_is_not_maids() {
        let e = env();
        add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
        let hand = format!(
            r#"{{"autoMemoryDirectory": "{}"}}"#,
            e.stores.join("work/memory").display()
        );
        add_store(&e.config, "work", &e.stores.join("work"), false).unwrap();
        seed_settings(&e, &hand);
        let out = install_claude(&e.home, &cfg(&e), &own(&e, None), true, false).unwrap();
        assert_eq!(out.item.unwrap().skipped.as_deref(), Some(OTHER_VALUE));
        uninstall_claude(&e.home, &own(&e, None), false).unwrap();
        assert_eq!(settings(&e), hand);
    }

    #[test]
    fn status_names_the_default_missing_stores_and_problems() {
        let e = env();
        add_store(&e.config, "personal", &e.stores.join("personal"), true).unwrap();
        add_store(&e.config, "work", &e.stores.join("work"), false).unwrap();
        fs::remove_dir(e.stores.join("work")).unwrap();
        other_part(&e, Some("x"), &[]);
        let lines = status(&e.home, &cfg(&e), &own(&e, None))
            .unwrap()
            .join("\n");
        assert!(lines.contains("store personal"), "{lines}");
        assert!(lines.contains("MISSING"), "{lines}");
        assert!(lines.contains("problem"), "{lines}");
        assert!(lines.contains("not set"), "{lines}");
        seed_settings(&e, r#"{"autoMemoryDirectory": "/x/memory"}"#);
        let other = status(&e.home, &cfg(&e), &own(&e, None))
            .unwrap()
            .join("\n");
        assert!(other.contains("-> /x/memory (not mAId's)"), "{other}");
    }

    // ── harvest ─────────────────────────────────────────────────

    fn memory(name: &str, body: &str) -> String {
        format!("---\nname: {name}\ndescription: about {name}\nmetadata:\n  type: feedback\n---\n\n{body}\n")
    }

    fn seed_project(e: &Env, project: &str, files: &[(&str, &str)], index: Option<&str>) {
        let dir = e.home.join(".claude/projects").join(project).join("memory");
        fs::create_dir_all(&dir).unwrap();
        for (f, body) in files {
            fs::write(dir.join(f), body).unwrap();
        }
        if let Some(i) = index {
            fs::write(dir.join(INDEX), i).unwrap();
        }
    }

    fn store(e: &Env) -> Store {
        Store {
            name: "personal".into(),
            path: e.stores.join("personal"),
            part: PART.into(),
        }
    }

    fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut out = BTreeMap::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(d) = stack.pop() {
            for entry in fs::read_dir(&d).unwrap() {
                let p = entry.unwrap().path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    out.insert(p.clone(), fs::read(&p).unwrap());
                }
            }
        }
        out
    }

    #[test]
    fn harvest_copies_carries_index_lines_and_leaves_originals() {
        let e = env();
        seed_project(
            &e,
            "-p1",
            &[("a.md", &memory("a", "A")), ("b.md", &memory("b", "B"))],
            Some("# Index\n\n- [Alpha](a.md) — the a one\n"),
        );
        let before = snapshot(&e.home);
        let r = harvest(&e.home, &e.home.join(".codex"), &store(&e)).unwrap();
        assert_eq!(r.copied.len(), 2);
        assert_eq!(snapshot(&e.home), before);
        let mem = e.stores.join("personal/memory");
        assert_eq!(
            fs::read_to_string(mem.join("a.md")).unwrap(),
            memory("a", "A")
        );
        let index = fs::read_to_string(mem.join(INDEX)).unwrap();
        assert_eq!(
            index,
            "- [Alpha](a.md) — the a one\n- [b](b.md) - about b\n"
        );
        assert!(r.notes.iter().any(|n| n.starts_with("created")));
    }

    #[test]
    fn a_second_harvest_changes_nothing() {
        let e = env();
        seed_project(&e, "-p1", &[("a.md", &memory("a", "A"))], None);
        harvest(&e.home, &e.home.join(".codex"), &store(&e)).unwrap();
        let first = snapshot(&e.stores);
        let r = harvest(&e.home, &e.home.join(".codex"), &store(&e)).unwrap();
        assert_eq!(snapshot(&e.stores), first);
        assert!(r.lines().contains(&"nothing new to harvest".to_string()));
        assert_eq!(r.present.len(), 1);
    }

    #[test]
    fn same_name_different_memories_both_land_deterministically() {
        let e = env();
        seed_project(
            &e,
            "-p1",
            &[("x.md", &memory("x", "one"))],
            Some("- [X1](x.md) - one\n"),
        );
        seed_project(
            &e,
            "-p2",
            &[("x.md", &memory("x", "two"))],
            Some("- [X2](x.md) - two\n"),
        );
        let r = harvest(&e.home, &e.home.join(".codex"), &store(&e)).unwrap();
        let mem = e.stores.join("personal/memory");
        assert!(fs::read_to_string(mem.join("x.md"))
            .unwrap()
            .contains("one"));
        assert!(fs::read_to_string(mem.join("x-2.md"))
            .unwrap()
            .contains("two"));
        assert_eq!(r.renamed.len(), 1);
        assert!(r.renamed[0].0.contains("-p2"));
        let index = fs::read_to_string(mem.join(INDEX)).unwrap();
        assert_eq!(index, "- [X1](x.md) - one\n- [X2](x-2.md) - two\n");
    }

    #[test]
    fn an_existing_index_line_is_not_repeated() {
        let e = env();
        let mem = e.stores.join("personal/memory");
        fs::create_dir_all(&mem).unwrap();
        fs::write(mem.join(INDEX), "- [mine](a.md) - kept").unwrap();
        fs::write(mem.join("a.md"), "other bytes").unwrap();
        seed_project(&e, "-p1", &[("b.md", &memory("b", "B"))], None);
        harvest(&e.home, &e.home.join(".codex"), &store(&e)).unwrap();
        let index = fs::read_to_string(mem.join(INDEX)).unwrap();
        assert_eq!(index, "- [mine](a.md) - kept\n- [b](b.md) - about b\n");
    }

    #[test]
    fn the_store_itself_is_not_a_source() {
        let e = env();
        let mem = e.stores.join("personal/memory");
        fs::create_dir_all(e.home.join(".claude/projects/-p1")).unwrap();
        fs::create_dir_all(&mem).unwrap();
        std::os::unix::fs::symlink(&mem, e.home.join(".claude/projects/-p1/memory")).unwrap();
        fs::write(mem.join("a.md"), memory("a", "A")).unwrap();
        let r = harvest(&e.home, &e.home.join(".codex"), &store(&e)).unwrap();
        assert!(r.copied.is_empty() && r.present.is_empty());
    }

    #[test]
    fn codex_memories_are_wrapped_as_reference_memories() {
        let e = env();
        let codex = e.home.join(".codex/memories");
        fs::create_dir_all(&codex).unwrap();
        fs::write(codex.join("MEMORY.md"), "# Task Group\n- fact\n").unwrap();
        fs::write(codex.join("raw_memories.md"), "queue").unwrap();
        let r = harvest(&e.home, &e.home.join(".codex"), &store(&e)).unwrap();
        assert_eq!(r.copied.len(), 1);
        let mem = e.stores.join("personal/memory");
        let text = fs::read_to_string(mem.join("codex-memory.md")).unwrap();
        assert!(text.starts_with("---\nname: codex-memory\n"));
        assert!(text.contains("  type: reference\n") && text.ends_with("- fact\n"));
        assert!(!mem.join("raw_memories.md").exists());
    }

    #[test]
    fn a_rerun_writes_the_index_line_an_interrupted_run_missed() {
        let e = env();
        seed_project(&e, "-p1", &[("a.md", &memory("a", "A"))], None);
        harvest(&e.home, &e.home.join(".codex"), &store(&e)).unwrap();
        let mem = e.stores.join("personal/memory");
        fs::remove_file(mem.join(INDEX)).unwrap();
        let r = harvest(&e.home, &e.home.join(".codex"), &store(&e)).unwrap();
        assert_eq!(r.present.len(), 1);
        assert_eq!(
            fs::read_to_string(mem.join(INDEX)).unwrap(),
            "- [a](a.md) - about a\n"
        );
    }

    #[test]
    fn a_generated_index_line_stays_one_line_and_one_link() {
        let body = "---\nname: a [b]\ndescription: |\n  two\n  lines\n---\nx\n";
        assert_eq!(
            generated_line(body.as_bytes(), "s"),
            "- [a b]({file}) - two lines"
        );
    }

    #[test]
    fn a_linked_index_is_written_through_and_new_files_are_0644() {
        use std::os::unix::fs::PermissionsExt;
        let e = env();
        let mem = e.stores.join("personal/memory");
        fs::create_dir_all(&mem).unwrap();
        let real = e.stores.join("index-elsewhere.md");
        fs::write(&real, "").unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o640)).unwrap();
        std::os::unix::fs::symlink(&real, mem.join(INDEX)).unwrap();
        seed_project(&e, "-p1", &[("a.md", &memory("a", "A"))], None);
        harvest(&e.home, &e.home.join(".codex"), &store(&e)).unwrap();
        assert!(fs::symlink_metadata(mem.join(INDEX))
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(fs::read_to_string(&real).unwrap().contains("](a.md)"));
        assert_eq!(
            fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert_eq!(
            fs::metadata(mem.join("a.md")).unwrap().permissions().mode() & 0o777,
            0o644
        );
    }

    #[test]
    fn an_unreadable_index_is_never_replaced() {
        let e = env();
        let mem = e.stores.join("personal/memory");
        fs::create_dir_all(&mem).unwrap();
        fs::write(mem.join(INDEX), [0xff, 0xfe, b'\n']).unwrap();
        seed_project(&e, "-p1", &[("a.md", &memory("a", "A"))], None);
        assert!(harvest(&e.home, &e.home.join(".codex"), &store(&e)).is_err());
        assert_eq!(fs::read(mem.join(INDEX)).unwrap(), [0xff, 0xfe, b'\n']);
    }

    #[test]
    fn a_long_index_is_warned_about() {
        let e = env();
        let mem = e.stores.join("personal/memory");
        fs::create_dir_all(&mem).unwrap();
        fs::write(mem.join(INDEX), "- line\n".repeat(INDEX_LINES)).unwrap();
        seed_project(&e, "-p1", &[("a.md", &memory("a", "A"))], None);
        let r = harvest(&e.home, &e.home.join(".codex"), &store(&e)).unwrap();
        assert!(r.notes.iter().any(|n| n.starts_with("warning")));
    }
}
