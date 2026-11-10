//! Parts: each repo's persona and bring-ins, as the ghost finds them in
//! what the shell has installed. A unit whose author declared it is the
//! author's own, in the part its `repository` names; a unit undeclared, or
//! declared only by an overlay, is borrowed and only reported. A part's
//! persona is the one its memory declares. See the initiative spec
//! `harness-core`, "What the ghost reads" and "The bring-in contract".

use crate::binding;
use crate::contract::{self, Declaration, Slot, Standard};
use crate::installed::{Installed, Kind, Origin, Unit};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Where a bring-in's manifest is, so its declaration is read from its slot.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Manifest {
    pub standard: Standard,
    pub path: PathBuf,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    /// A git project repo.
    Project,
    /// A git store the person owns.
    Store,
    /// A directory on this machine.
    Dir,
    Vendor,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct Source {
    pub kind: SourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    /// The pinned revision: a plugin's content-hash `version` for what it
    /// ships, a commit for a git marketplace's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rev: Option<String>,
    /// The date it was installed, `YYYY-MM-DD`, when the shell says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase", tag = "kind")]
pub enum Tenure {
    #[default]
    Own,
    Borrowed {
        from: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        since: Option<String>,
    },
}

/// How a finding about this bring-in is applied.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Route {
    Write,
    Backlog,
    /// A worktree to turn into a PR on the source.
    Worktree,
    Report,
}

/// One bring-in as its part has it.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Debug)]
pub struct BringIn {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<Manifest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<Source>,
    #[serde(default)]
    pub tenure: Tenure,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route: Option<Route>,
    /// The config values bound here. A secret's value is `env:<NAME>`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, String>,
}

impl BringIn {
    /// The route a finding takes: borrowed is always report only; otherwise
    /// the declared one, or the source's default.
    pub fn route(&self) -> Route {
        if matches!(self.tenure, Tenure::Borrowed { .. }) {
            return Route::Report;
        }
        self.route
            .unwrap_or(match self.source.as_ref().map(|s| s.kind) {
                Some(SourceKind::Project) => Route::Worktree,
                Some(SourceKind::Store | SourceKind::Dir) => Route::Write,
                Some(SourceKind::Vendor) | None => Route::Report,
            })
    }
}

/// One part: a repo's persona and bring-ins.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub part: String,
    pub persona: Option<String>,
    /// Whether any of its units declares itself; without that, its persona
    /// is not known and nothing of it is read for a declaration.
    pub declared: bool,
    /// Each bring-in, or its name (or `#<index>`) and why it was not taken.
    pub bring_ins: Vec<Result<BringIn, (String, String)>>,
}

/// Something the ghost would not take, and why.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Refused {
    pub file: String,
    pub why: String,
}

/// The part of units installed with no declaration and no marketplace.
pub const LOCAL: &str = "local";

/// Every part the enabled units make: those that declare, then those that
/// do not, each in name order.
pub fn of(installed: &dyn Installed, here: Option<&Path>) -> anyhow::Result<Vec<Part>> {
    let units: Vec<Unit> = installed
        .units(here)?
        .into_iter()
        .filter(|u| u.enabled)
        .collect();
    let shipped = Shipped::by(&units);
    let mut found: Vec<Found> = vec![];
    for u in &units {
        found.extend(bring_ins_of(u, &shipped));
    }
    Ok(group(found, installed))
}

/// A bring-in as found, before it is bound and grouped.
struct Found {
    part: String,
    /// Its author's declaration, or the overlay's for a borrowed unit.
    declared: Option<Declaration>,
    /// Whether the unit's own manifest holds a declaration, readable or
    /// not: such a unit is its author's, not the plugin's it sits in.
    claims: bool,
    record: BringIn,
    registration: Option<crate::installed::Registration>,
}

/// What the enabled plugins ship for units outside them: each MCP server's
/// declaration, by registered name, at `servers/<name>/server.json`, and an
/// overlay for each unit they borrow, at `overlays/<name>/<manifest>`; each
/// with the shipping plugin's repository.
struct Shipped {
    servers: BTreeMap<String, (PathBuf, Option<Home>)>,
    overlays: BTreeMap<String, (Manifest, Option<Home>)>,
}

impl Shipped {
    fn by(units: &[Unit]) -> Shipped {
        let (mut servers, mut overlays) = (BTreeMap::new(), BTreeMap::new());
        for u in units.iter().filter(|u| u.kind == Kind::Plugin) {
            let Some(dir) = &u.dir else { continue };
            let repo = u.manifest.as_deref().and_then(repository_of).map(|h| Home {
                installed: u.from.as_ref().and_then(|o| o.since.clone()),
                ..h
            });
            for (name, sub) in subdirs(&dir.join("servers")) {
                let file = sub.join("server.json");
                if file.is_file() {
                    servers.entry(name).or_insert((file, repo.clone()));
                }
            }
            for (name, sub) in subdirs(&dir.join("overlays")) {
                let manifest = [
                    ("plugin.json", Standard::Plugin),
                    ("SKILL.md", Standard::Skill),
                    ("server.json", Standard::Mcp),
                ]
                .into_iter()
                .map(|(f, standard)| Manifest {
                    standard,
                    path: sub.join(f),
                })
                .find(|m| m.path.is_file());
                if let Some(m) = manifest {
                    overlays.entry(name).or_insert((m, repo.clone()));
                }
            }
        }
        Shipped { servers, overlays }
    }
}

fn subdirs(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| (e.file_name().to_string_lossy().into_owned(), e.path()))
        .filter(|(n, p)| !n.starts_with('.') && p.is_dir())
        .collect();
    out.sort();
    out
}

/// Where a plugin's units come from: its manifest's standard `repository`,
/// and its `version` as the build of it installed (a plugin's version is
/// stamped when it is built).
#[derive(Clone, Debug)]
struct Home {
    repository: String,
    revision: Option<String>,
    installed: Option<String>,
}

fn repository_of(manifest: &Path) -> Option<Home> {
    let v: Value = serde_json::from_str(&contract::read_small(manifest).ok()?).ok()?;
    let repository = v["repository"]
        .as_str()
        .or_else(|| v["repository"]["url"].as_str())?
        .to_string();
    Some(Home {
        repository,
        revision: v["version"].as_str().map(str::to_string),
        installed: None,
    })
}

fn standard_of(kind: Kind) -> Standard {
    match kind {
        Kind::Skill | Kind::Agent => Standard::Skill,
        Kind::Mcp => Standard::Mcp,
        Kind::Plugin => Standard::Plugin,
    }
}

/// What one unit brings in. A plugin brings in each skill of it that
/// declares itself, and itself when it declares or is overlaid; with
/// neither, none of its skills declared and no declaration shipped for a
/// unit outside it, it is one borrowed unit.
fn bring_ins_of(u: &Unit, shipped: &Shipped) -> Vec<Found> {
    let own = |m: &Path| Manifest {
        standard: standard_of(u.kind),
        path: m.to_path_buf(),
    };
    if u.kind != Kind::Plugin {
        let manifest = match (u.kind, &u.manifest) {
            (Kind::Mcp, _) => shipped
                .servers
                .get(&u.name)
                .map(|(p, r)| (own(p), r.clone())),
            (_, Some(m)) => Some((own(m), None)),
            (_, None) => None,
        };
        return vec![one(u, &u.name, manifest, None, shipped)];
    }
    let repo = u.manifest.as_deref().and_then(repository_of).map(|h| Home {
        installed: u.from.as_ref().and_then(|o| o.since.clone()),
        ..h
    });
    let mut out: Vec<Found> = u
        .contents
        .iter()
        .filter_map(|s| {
            let m = s.manifest.as_ref()?;
            let found = one(
                s,
                &s.name,
                Some((own_skill(m), repo.clone())),
                u.from.as_ref(),
                shipped,
            );
            found.claims.then_some(found)
        })
        .collect();
    let name = u.name.split('@').next().unwrap_or(&u.name).to_string();
    let me = one(
        u,
        &name,
        u.manifest.as_deref().map(|m| (own(m), None)),
        None,
        shipped,
    );
    let ships = u.dir.as_deref().is_some_and(|d| {
        !subdirs(&d.join("servers")).is_empty() || !subdirs(&d.join("overlays")).is_empty()
    });
    if me.claims || shipped.overlays.contains_key(&name) || (out.is_empty() && !ships) {
        out.push(me);
    }
    out
}

fn own_skill(m: &Path) -> Manifest {
    Manifest {
        standard: Standard::Skill,
        path: m.to_path_buf(),
    }
}

/// One bring-in from a unit: declared by its author (own, in the part its
/// repository names), or else by an overlay a plugin ships, or else not at
/// all (both borrowed). `manifest` is the unit's own, with where the
/// plugin it sits in comes from; `origin_of_plugin` that plugin's
/// marketplace, for a skill inside it.
fn one(
    u: &Unit,
    name: &str,
    manifest: Option<(Manifest, Option<Home>)>,
    origin_of_plugin: Option<&Origin>,
    shipped: &Shipped,
) -> Found {
    let origin = u.from.as_ref().or(origin_of_plugin);
    let (manifest, inherit) = match manifest {
        Some((m, r)) => (Some(m), r),
        None => (None, None),
    };
    let slot = manifest
        .as_ref()
        .map(|m| contract::read(m.standard, &m.path));
    let borrowed = |unit: Option<Manifest>, part: String| Found {
        part,
        declared: None,
        claims: false,
        record: BringIn {
            name: name.into(),
            unit,
            source: origin.map(|o| Source {
                kind: SourceKind::Vendor,
                at: o.repo.clone(),
                rev: o.rev.clone(),
                installed: o.since.clone(),
            }),
            tenure: Tenure::Borrowed {
                from: origin.map_or(LOCAL.into(), |o| o.marketplace.clone()),
                since: origin.and_then(|o| o.since.clone()),
            },
            route: None,
            values: BTreeMap::new(),
        },
        registration: u.registration.clone(),
    };
    match slot {
        Some(Ok(Slot::Declared(d))) => {
            let since = origin.and_then(|o| o.since.clone());
            let (repository, revision, installed) = match (&d.repository, inherit) {
                (Some(r), _) => (Some(r.clone()), None, since),
                (None, Some(h)) => (Some(h.repository), h.revision, h.installed.or(since)),
                (None, None) => (None, None, since),
            };
            let name = d.name.clone().unwrap_or_else(|| name.into());
            Found {
                part: repository.as_deref().map_or(LOCAL.into(), part_name),
                record: BringIn {
                    name,
                    unit: manifest,
                    source: repository.map(|r| Source {
                        kind: SourceKind::Project,
                        at: Some(r),
                        rev: revision,
                        installed,
                    }),
                    tenure: Tenure::Own,
                    route: None,
                    values: BTreeMap::new(),
                },
                declared: Some(*d),
                claims: true,
                registration: u.registration.clone(),
            }
        }
        // Unreadable: a unit that tried to declare is kept with its
        // manifest, so the ghost says why; one that did not is the shell's.
        Some(Err(_)) if names_the_contract(manifest.as_ref()) => Found {
            part: inherit.map_or(LOCAL.into(), |h| part_name(&h.repository)),
            claims: true,
            ..borrowed(manifest, String::new())
        },
        _ => match shipped.overlays.get(name) {
            Some((overlay, repo)) => Found {
                part: repo
                    .as_ref()
                    .map_or(LOCAL.into(), |h| part_name(&h.repository)),
                declared: match contract::read(overlay.standard, &overlay.path) {
                    Ok(Slot::Declared(d)) => Some(*d),
                    _ => None,
                },
                ..borrowed(Some(overlay.clone()), String::new())
            },
            None => {
                let part = origin.map_or(LOCAL.into(), |o| o.marketplace.clone());
                borrowed(manifest, part)
            }
        },
    }
}

/// Whether a manifest mentions the contract's namespace at all.
fn names_the_contract(m: Option<&Manifest>) -> bool {
    m.and_then(|m| contract::read_small(&m.path).ok())
        .is_some_and(|t| {
            t.contains(&format!("{}.", contract::NAMESPACE))
                || t.contains(&format!("\"{}\"", contract::NAMESPACE))
        })
}

/// A part's name: its repository's last path segment, `.git` dropped.
pub fn part_name(repository: &str) -> String {
    let path = repository.trim_end_matches('/');
    let last = path.rsplit(['/', ':']).next().unwrap_or(path);
    last.strip_suffix(".git").unwrap_or(last).to_string()
}

/// Bind each declared bring-in's values, and gather bring-ins into parts,
/// each part's persona its memory's.
fn group(found: Vec<Found>, installed: &dyn Installed) -> Vec<Part> {
    let mut parts: BTreeMap<String, Part> = BTreeMap::new();
    for f in found {
        let part = parts.entry(f.part.clone()).or_insert_with(|| Part {
            part: f.part.clone(),
            persona: None,
            declared: false,
            bring_ins: vec![],
        });
        let mut record = f.record;
        // A unit that declares, even unreadably, says the part is there.
        part.declared |= f.claims || f.declared.is_some();
        if let Some(d) = &f.declared {
            if part.persona.is_none() && is_memory(d) {
                part.persona = d.persona.clone();
            }
            record.values = binding::values(d, f.registration.as_ref(), installed);
        }
        let taken = part
            .bring_ins
            .iter()
            .any(|b| b.as_ref().map_or_else(|(n, _)| n, |b| &b.name) == &record.name);
        if !taken {
            part.bring_ins.push(Ok(record));
        }
    }
    let mut parts: Vec<Part> = parts
        .into_values()
        .map(|mut p| {
            p.bring_ins.sort_by(|a, b| key(a).cmp(key(b)));
            p
        })
        .collect();
    // Parts that say what they are first, each in name order.
    parts.sort_by_key(|p| !p.declared);
    parts
}

/// A memory bring-in: one whose permanent tier is its store. Only a
/// memory says whose the part is.
fn is_memory(d: &Declaration) -> bool {
    d.state.as_ref().is_some_and(|s| s.permanent.is_some())
}

fn key(b: &Result<BringIn, (String, String)>) -> &str {
    match b {
        Ok(b) => &b.name,
        Err((n, _)) => n,
    }
}

impl Part {
    /// A part in its JSON form (`part`, `persona`, `bring_ins` as records),
    /// for fixtures and tests; nothing installed is read this way.
    pub fn from_json(v: &Value) -> Result<Part, String> {
        let part = v["part"]
            .as_str()
            .ok_or("part is not a string")?
            .to_string();
        let persona = match &v["persona"] {
            Value::Null => None,
            Value::String(s) => Some(s.clone()),
            _ => return Err("persona is not a string".into()),
        };
        let bring_ins = match &v["bring_ins"] {
            Value::Null => vec![],
            Value::Array(items) => items
                .iter()
                .enumerate()
                .map(|(i, b)| record(i, b))
                .collect(),
            _ => return Err("bring_ins is not an array".into()),
        };
        Ok(Part {
            part,
            persona,
            declared: true,
            bring_ins,
        })
    }
}

fn record(i: usize, v: &Value) -> Result<BringIn, (String, String)> {
    // A null field is absent.
    let v = &match v {
        Value::Object(m) => Value::Object(
            m.iter()
                .filter(|(_, x)| !x.is_null())
                .map(|(k, x)| (k.clone(), x.clone()))
                .collect(),
        ),
        other => other.clone(),
    };
    let name = v["name"]
        .as_str()
        .map_or_else(|| format!("#{i}"), str::to_string);
    serde_json::from_value(v.clone()).map_err(|e| (name, e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::installed::Registration;
    use serde_json::json;
    use tempfile::TempDir;

    fn skill(dir: &Path, name: &str, meta: &str) -> PathBuf {
        let d = dir.join(name);
        fs::create_dir_all(&d).unwrap();
        let p = d.join("SKILL.md");
        fs::write(
            &p,
            format!("---\nname: {name}\ndescription: about {name}\nmetadata:\n{meta}---\n"),
        )
        .unwrap();
        p
    }

    const TURN: &str =
        "  my-harness.contract: \"1\"\n  my-harness.scope: turn\n  my-harness.job: j\n";

    fn json_file(p: &Path, v: Value) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, v.to_string()).unwrap();
    }

    /// A plugin dir with a manifest naming `repo`, these skills, a memory
    /// server's declaration and an overlay for `docs`.
    fn plugin(dir: &Path, repo: &str) -> Unit {
        let at = dir.join("plugin");
        json_file(
            &at.join(".claude-plugin/plugin.json"),
            json!({"name": "kit", "repository": repo}),
        );
        let s = skill(&at.join("skills"), "notes", TURN);
        skill(&at.join("skills"), "plain", "");
        json_file(
            &at.join("servers/mem/server.json"),
            json!({"name": "mem", "_meta": {"my-harness": {"contract": 1, "scope": "session",
                "job": "memory", "persona": "personal",
                "config": {"store": {"about": "s", "bind": "arg:--store"}, "serve": {"about": "c", "bind": "command"}},
                "state": {"permanent": "store", "promote": "off"}}}}),
        );
        json_file(
            &at.join("overlays/docs/plugin.json"),
            json!({"name": "docs", "extensions": {"my-harness": {"contract": 1, "scope": "turn", "job": "docs"}}}),
        );
        let mut p = Unit::new(Kind::Plugin, "kit@home");
        p.dir = Some(at.clone());
        p.manifest = Some(at.join(".claude-plugin/plugin.json"));
        let mut notes = Unit::new(Kind::Skill, "notes");
        notes.manifest = Some(s);
        let mut plain = Unit::new(Kind::Skill, "plain");
        plain.manifest = Some(at.join("skills/plain/SKILL.md"));
        p.contents = vec![notes, plain];
        p
    }

    fn vendor() -> Unit {
        let mut docs = Unit::new(Kind::Plugin, "docs@vendor");
        docs.from = Some(Origin {
            marketplace: "vendor".into(),
            repo: Some("https://example.org/vendor.git".into()),
            rev: Some("v1".into()),
            since: Some("2026-10-31".into()),
        });
        docs.contents = vec![Unit::new(Kind::Skill, "pdf")];
        docs
    }

    fn memory() -> Unit {
        let mut m = Unit::new(Kind::Mcp, "mem");
        m.registration = Some(Registration {
            command: "/bin/pensieve".into(),
            args: vec!["--store".into(), "/s".into()],
            env: BTreeMap::new(),
        });
        m
    }

    fn names(p: &Part) -> Vec<&str> {
        p.bring_ins.iter().map(key).collect()
    }

    fn get<'a>(p: &'a Part, name: &str) -> &'a BringIn {
        p.bring_ins
            .iter()
            .flatten()
            .find(|b| b.name == name)
            .unwrap()
    }

    #[test]
    fn a_plugins_declared_skills_and_its_shipped_server_make_its_repos_part_with_the_memorys_persona(
    ) {
        let dir = TempDir::new().unwrap();
        let units = vec![
            plugin(dir.path(), "https://example.org/me/kit.git"),
            memory(),
        ];
        let parts = of(&units, None).unwrap();
        assert_eq!(parts.len(), 1);
        let p = &parts[0];
        assert_eq!(
            (p.part.as_str(), p.persona.as_deref()),
            ("kit", Some("personal"))
        );
        assert_eq!(
            names(p),
            ["mem", "notes"],
            "an undeclared skill inside a plugin is the plugin's"
        );
        let notes = get(p, "notes");
        assert_eq!(notes.tenure, Tenure::Own);
        assert_eq!(
            notes.source.as_ref().unwrap().at.as_deref(),
            Some("https://example.org/me/kit.git")
        );
        assert_eq!(notes.route(), Route::Worktree);
        let mem = get(p, "mem");
        assert_eq!(mem.values["store"], "/s");
        assert_eq!(mem.values["serve"], r#"["/bin/pensieve","--store","/s"]"#);
    }

    #[test]
    fn an_overlaid_vendor_plugin_is_borrowed_from_its_marketplace_in_the_overlays_part() {
        let dir = TempDir::new().unwrap();
        let units = vec![plugin(dir.path(), "https://example.org/me/kit"), vendor()];
        let parts = of(&units, None).unwrap();
        let docs = get(&parts[0], "docs");
        assert_eq!(
            docs.tenure,
            Tenure::Borrowed {
                from: "vendor".into(),
                since: Some("2026-10-31".into())
            }
        );
        assert_eq!(
            docs.unit.as_ref().unwrap().path,
            dir.path().join("plugin/overlays/docs/plugin.json")
        );
        assert_eq!(docs.source.as_ref().unwrap().rev.as_deref(), Some("v1"));
        assert_eq!(docs.route(), Route::Report);
        assert!(!names(&parts[0]).contains(&"pdf"));
    }

    #[test]
    fn a_unit_installed_by_hand_joins_its_own_part_when_it_declares_itself() {
        let dir = TempDir::new().unwrap();
        let mut hand = Unit::new(Kind::Skill, "garden");
        hand.manifest = Some(skill(
            dir.path(),
            "garden",
            &format!("{TURN}  my-harness.repository: https://example.org/me/garden\n"),
        ));
        let mut loose = Unit::new(Kind::Skill, "loose");
        loose.manifest = Some(skill(dir.path(), "loose", ""));
        let parts = of(&vec![hand, loose], None).unwrap();
        let got: Vec<(&str, Vec<&str>, bool)> = parts
            .iter()
            .map(|p| (p.part.as_str(), names(p), p.declared))
            .collect();
        assert_eq!(
            got,
            [
                ("garden", vec!["garden"], true),
                ("local", vec!["loose"], false)
            ]
        );
        assert_eq!(get(&parts[0], "garden").tenure, Tenure::Own);
        assert!(matches!(
            get(&parts[1], "loose").tenure,
            Tenure::Borrowed { .. }
        ));
    }

    #[test]
    fn only_a_memory_says_whose_the_part_is_and_an_unreadable_part_still_shows() {
        let dir = TempDir::new().unwrap();
        let mut early = Unit::new(Kind::Skill, "aaa");
        early.manifest = Some(skill(
            dir.path(),
            "aaa",
            &format!("{TURN}  my-harness.persona: someone\n  my-harness.repository: https://example.org/me/kit\n"),
        ));
        let units = vec![
            early,
            plugin(dir.path(), "https://example.org/me/kit"),
            memory(),
        ];
        let parts = of(&units, None).unwrap();
        assert_eq!(parts[0].persona.as_deref(), Some("personal"));
        let mut next = Unit::new(Kind::Skill, "next");
        next.manifest = Some(skill(
            dir.path(),
            "next",
            "  my-harness.contract: \"2\"\n  my-harness.scope: turn\n  my-harness.job: j\n",
        ));
        let parts = of(&vec![next], None).unwrap();
        assert!(
            parts[0].declared,
            "a part whose only declaration is unreadable is still reported"
        );
    }

    #[test]
    fn a_disabled_unit_and_its_plugins_declarations_are_not_read() {
        let dir = TempDir::new().unwrap();
        let mut p = plugin(dir.path(), "https://example.org/me/kit");
        p.enabled = false;
        let parts = of(&vec![p, memory()], None).unwrap();
        assert_eq!(parts[0].part, LOCAL);
        assert_eq!(names(&parts[0]), ["mem"]);
        assert!(get(&parts[0], "mem").unit.is_none());
    }

    #[test]
    fn a_plugin_names_its_own_bring_in_by_its_declaration() {
        let dir = TempDir::new().unwrap();
        let at = dir.path().join("p");
        json_file(
            &at.join(".claude-plugin/plugin.json"),
            json!({"name": "maid", "repository": "https://example.org/me/maid",
                "extensions": {"my-harness": {"contract": 1, "name": "claude-adapter", "scope": "session", "job": "adapter"}}}),
        );
        let mut u = Unit::new(Kind::Plugin, "maid@maid");
        u.dir = Some(at.clone());
        u.manifest = Some(at.join(".claude-plugin/plugin.json"));
        let parts = of(&vec![u], None).unwrap();
        assert_eq!(names(&parts[0]), ["claude-adapter"]);
    }

    #[test]
    fn a_plugin_with_nothing_declared_is_one_borrowed_unit() {
        let parts = of(&vec![vendor()], None).unwrap();
        assert_eq!(
            (parts[0].part.as_str(), names(&parts[0])),
            ("vendor", vec!["docs"])
        );
        assert!(!parts[0].declared);
    }

    #[test]
    fn a_part_is_named_by_its_repositorys_last_segment() {
        for (repo, name) in [
            ("https://github.com/me/mAId", "mAId"),
            ("https://example.org/x/kit.git/", "kit"),
            ("github.com:me/notes.git", "notes"),
        ] {
            assert_eq!(part_name(repo), name);
        }
    }

    #[test]
    fn a_part_reads_from_its_json_form() {
        let p = Part::from_json(&json!({"part": "w", "persona": "work", "bring_ins": [
            {"name": "mem", "unit": {"standard": "mcp", "path": "/u/server.json"},
             "source": {"kind": "store", "at": "git@example.org:me/mem", "rev": "abc"},
             "tenure": {"kind": "borrowed", "from": "a friend", "since": "2026-10-01"},
             "route": "write", "values": {"store": "/s"}, "later": "ignored"},
            {"name": "bad", "tenure": {"kind": "rented"}},
            {"route": "write"}]}))
        .unwrap();
        assert_eq!(p.persona.as_deref(), Some("work"));
        let b = p.bring_ins[0].clone().unwrap();
        assert_eq!(b.unit.unwrap().standard, Standard::Mcp);
        assert_eq!(b.values["store"], "/s");
        assert_eq!(p.bring_ins[1].as_ref().unwrap_err().0, "bad");
        assert_eq!(p.bring_ins[2].as_ref().unwrap_err().0, "#2");
        assert!(Part::from_json(&json!({"part": "x", "persona": 1})).is_err());
    }

    #[test]
    fn route_defaults_from_the_source_and_borrowed_is_report_only() {
        let at = |kind| BringIn {
            name: "b".into(),
            unit: None,
            source: Some(Source {
                kind,
                at: None,
                rev: None,
                installed: None,
            }),
            tenure: Tenure::Own,
            route: None,
            values: BTreeMap::new(),
        };
        assert_eq!(at(SourceKind::Project).route(), Route::Worktree);
        assert_eq!(at(SourceKind::Store).route(), Route::Write);
        assert_eq!(at(SourceKind::Dir).route(), Route::Write);
        assert_eq!(at(SourceKind::Vendor).route(), Route::Report);
        let declared = BringIn {
            route: Some(Route::Backlog),
            ..at(SourceKind::Project)
        };
        assert_eq!(declared.route(), Route::Backlog);
        let borrowed = BringIn {
            tenure: Tenure::Borrowed {
                from: "x".into(),
                since: None,
            },
            ..declared
        };
        assert_eq!(borrowed.route(), Route::Report);
    }
}
