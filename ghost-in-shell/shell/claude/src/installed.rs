//! What Claude Code has installed, read from its own records under the
//! resolved config dir: the plugin registry and each plugin's install path,
//! `enabledPlugins` across user, project and local settings, the MCP
//! servers in `.claude.json` and a project's `.mcp.json`, and the skills
//! and agents in the user's and the project's dirs. What it derived is
//! cached under the ghost's state, keyed on those files' modification
//! times, since `.claude.json` can be large and the hooks run on every
//! prompt. See the initiative spec `harness-core`, "What the ghost reads".

use ghost::installed::{Installed, Kind, Origin, Registration, Unit};
use plain::claude_config::ConfigDir;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Claude Code's installs, at its config dir (none without one).
pub struct Claude {
    pub config: Option<ConfigDir>,
    /// Where what was derived is kept; none reads afresh every time.
    pub cache: Option<PathBuf>,
    /// The environment units run in, when not this process's own.
    pub vars: Option<BTreeMap<String, String>>,
}

impl Claude {
    /// The units, and the records whose shape is not understood: from the
    /// cache while no record changed, else read and cached.
    fn read(&self, here: Option<&Path>) -> io::Result<Read> {
        let Some(config) = &self.config else {
            return Ok(Read::default());
        };
        let at = Records::of(config, here);
        let stamps = at.stamps();
        let cached = self.cache.as_ref().map(|c| c.join(at.key()));
        if let Some(read) = cached.as_deref().and_then(|c| read_cache(c, &stamps)) {
            return Ok(read);
        }
        let read = Read {
            units: at.units(here)?,
            not_understood: at.shapes(),
        };
        if let Some(c) = cached {
            // A cache that cannot be written only costs the next call a read.
            let _ = write_cache(&c, &stamps, &read);
        }
        Ok(read)
    }
}

impl Installed for Claude {
    fn units(&self, here: Option<&Path>) -> anyhow::Result<Vec<Unit>> {
        Ok(self.read(here)?.units)
    }

    fn not_understood(&self, here: Option<&Path>) -> Vec<String> {
        self.read(here)
            .map(|r| r.not_understood)
            .unwrap_or_default()
    }

    fn var(&self, name: &str) -> Option<String> {
        match &self.vars {
            Some(v) => v.get(name).cloned(),
            None => std::env::var(name).ok(),
        }
        .filter(|v| !v.is_empty())
    }
}

/// The files Claude Code keeps what is installed in, for one location.
struct Records {
    dir: PathBuf,
    global: PathBuf,
    project: Option<PathBuf>,
}

impl Records {
    fn of(config: &ConfigDir, here: Option<&Path>) -> Records {
        Records {
            dir: config.dir(),
            global: config.global(),
            project: here.filter(|h| h.is_absolute()).map(Path::to_path_buf),
        }
    }

    fn registry(&self) -> PathBuf {
        self.dir.join("plugins/installed_plugins.json")
    }

    fn marketplaces(&self) -> PathBuf {
        self.dir.join("plugins/known_marketplaces.json")
    }

    /// Settings, least specific first: user, then the project's, then
    /// its local ones.
    fn settings(&self) -> Vec<PathBuf> {
        let mut out = vec![self.dir.join("settings.json")];
        if let Some(p) = &self.project {
            out.push(p.join(".claude/settings.json"));
            out.push(p.join(".claude/settings.local.json"));
        }
        out
    }

    /// Dirs whose entries are units: skills and agents, the user's and the
    /// project's.
    fn dirs(&self) -> Vec<(Kind, PathBuf)> {
        let mut out = vec![
            (Kind::Skill, self.dir.join("skills")),
            (Kind::Agent, self.dir.join("agents")),
        ];
        if let Some(p) = &self.project {
            out.push((Kind::Skill, p.join(".claude/skills")));
            out.push((Kind::Agent, p.join(".claude/agents")));
        }
        out
    }

    /// Every file and dir read, each with its modification time.
    fn stamps(&self) -> Vec<(PathBuf, Option<u128>)> {
        let mut paths = vec![self.registry(), self.marketplaces(), self.global.clone()];
        paths.extend(self.settings());
        if let Some(p) = &self.project {
            paths.push(p.join(".mcp.json"));
        }
        paths.extend(self.dirs().into_iter().map(|(_, d)| d));
        // A directory marketplace through a link (a profile) is another one
        // once the link moves, though no file Claude Code wrote changed.
        let markets = json(&self.marketplaces());
        paths.extend(
            markets
                .as_object()
                .into_iter()
                .flatten()
                .filter(|(_, m)| m["source"]["source"] == "directory")
                .filter_map(|(_, m)| fs::canonicalize(m["source"]["path"].as_str()?).ok()),
        );
        paths
            .into_iter()
            .map(|p| {
                let t = fs::metadata(&p)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos());
                (p, t)
            })
            .collect()
    }

    /// The cache file's name for this config dir and location.
    fn key(&self) -> String {
        let mut h = DefaultHasher::new();
        (&self.dir, &self.global, &self.project).hash(&mut h);
        format!("{:016x}.json", h.finish())
    }

    /// Each record that exists and whose shape is not the one this reader
    /// takes. Its entries are still read leniently.
    fn shapes(&self) -> Vec<String> {
        let global = json(&self.global);
        let version = global["lastReleaseNotesSeen"]
            .as_str()
            .map_or("Claude Code, its version not known".into(), |v| {
                format!("Claude Code {v}")
            });
        let mut checks: Vec<(PathBuf, Shape)> = vec![
            (self.registry(), |v| {
                v["version"] == 2 && object_of(&v["plugins"], Value::is_array)
            }),
            (self.marketplaces(), |v| {
                object_of(v, |m| m["source"].is_object())
            }),
            (self.global.clone(), |v| {
                v.is_object()
                    && (v["mcpServers"].is_null() || v["mcpServers"].is_object())
                    && (v["projects"].is_null() || v["projects"].is_object())
            }),
        ];
        if let Some(p) = &self.project {
            checks.push((p.join(".mcp.json"), |v| v["mcpServers"].is_object()));
        }
        checks
            .into_iter()
            .filter(|(file, _)| file.is_file())
            .filter(|(file, ok)| !raw(file).is_some_and(|v| ok(&v)))
            .map(|(file, _)| format!("{}: shape not understood ({version})", file.display()))
            .collect()
    }

    fn units(&self, here: Option<&Path>) -> io::Result<Vec<Unit>> {
        let settings: Vec<Value> = self.settings().iter().map(|p| json(p)).collect();
        let global = json(&self.global);
        let mut units = self.plugins(&settings, here);
        units.extend(self.servers(&global, &settings));
        for (kind, dir) in self.dirs() {
            units.extend(match kind {
                Kind::Skill => skills(&dir),
                _ => agents(&dir),
            });
        }
        Ok(units)
    }

    /// Each plugin installed for the user, or for this project, enabled
    /// by the most specific setting that names it.
    fn plugins(&self, settings: &[Value], here: Option<&Path>) -> Vec<Unit> {
        let registry = json(&self.registry());
        let markets = json(&self.marketplaces());
        let mut out = vec![];
        for (id, installs) in registry["plugins"].as_object().into_iter().flatten() {
            // This project's own install first, else the user's.
            let installs: Vec<&Value> = installs.as_array().into_iter().flatten().collect();
            let install = installs
                .iter()
                .find(|i| {
                    matches!((i["projectPath"].as_str(), here), (Some(p), Some(h)) if Path::new(p) == h)
                })
                .or_else(|| installs.iter().find(|i| i["scope"] == "user"));
            let Some(install) = install else { continue };
            let Some(installed) = install["installPath"].as_str().map(PathBuf::from) else {
                continue;
            };
            let enabled = settings
                .iter()
                .rev()
                .find_map(|s| s["enabledPlugins"][id].as_bool())
                .unwrap_or(false);
            let marketplace = id.split_once('@').map(|(_, m)| m.to_string());
            // A directory marketplace's plugin is run from that directory,
            // so a profile link that moves is followed; others from the copy.
            let path = marketplace
                .as_deref()
                .and_then(|m| live(&markets[m]["source"], id))
                .unwrap_or(installed);
            let mut u = Unit::new(Kind::Plugin, id.clone());
            // A plugin a marketplace describes may have no manifest of its own.
            u.manifest = Some(path.join(".claude-plugin/plugin.json")).filter(|m| m.is_file());
            u.from = marketplace.map(|m| Origin {
                repo: repo_of(&markets[&m]["source"]),
                rev: install["gitCommitSha"].as_str().map(str::to_string),
                since: install["installedAt"]
                    .as_str()
                    .and_then(|t| t.get(..10))
                    .map(str::to_string),
                marketplace: m,
            });
            u.contents = skills(&path.join("skills"));
            u.dir = Some(path);
            u.enabled = enabled;
            out.push(u);
        }
        out
    }

    /// The MCP servers registered for the user, for this project locally,
    /// and in the project's `.mcp.json` unless disabled there.
    ///
    /// One name at several scopes is the one Claude Code runs: local, then
    /// project, then user. A project's `.mcp.json` server is on only once
    /// the person approved it, as Claude Code asks them to.
    fn servers(&self, global: &Value, settings: &[Value]) -> Vec<Unit> {
        let mut by_name: BTreeMap<String, Unit> = BTreeMap::new();
        let mut put = |units: Vec<Unit>| {
            for u in units {
                by_name.insert(u.name.clone(), u);
            }
        };
        put(registered(&global["mcpServers"]));
        if let Some(p) = &self.project {
            let local = &global["projects"][p.display().to_string()];
            let names = |v: &Value| -> Vec<String> {
                v.as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            };
            let on = names(&local["enabledMcpjsonServers"]);
            let off = names(&local["disabledMcpjsonServers"]);
            // Approval is the person's: the user's settings, the project's
            // local ones and Claude's own state, never the settings a repo
            // commits for itself.
            let personal: Vec<&Value> = [settings.first(), settings.get(2)]
                .into_iter()
                .flatten()
                .collect();
            let all = personal
                .iter()
                .any(|s| s["enableAllProjectMcpServers"] == true);
            let approved: Vec<String> = personal
                .iter()
                .flat_map(|s| names(&s["enabledMcpjsonServers"]))
                .chain(on)
                .collect();
            let mut shared = registered(&json(&p.join(".mcp.json"))["mcpServers"]);
            for u in &mut shared {
                u.enabled = (all || approved.contains(&u.name)) && !off.contains(&u.name);
            }
            // An unapproved project server is not what Claude Code runs,
            // so it leaves a user server of its name standing.
            put(shared.into_iter().filter(|u| u.enabled).collect());
            put(registered(&local["mcpServers"]));
        }
        by_name.into_values().collect()
    }
}

/// Where a directory marketplace keeps the plugin `id`: the source its
/// `marketplace.json` names, under the directory.
fn live(source: &Value, id: &str) -> Option<PathBuf> {
    if source["source"].as_str()? != "directory" {
        return None;
    }
    let dir = PathBuf::from(source["path"].as_str()?);
    let name = id.split('@').next()?;
    let market = json(&dir.join(".claude-plugin/marketplace.json"));
    let rel = market["plugins"]
        .as_array()?
        .iter()
        .find(|p| p["name"] == name)?["source"]
        .as_str()?
        .to_string();
    // Only a plugin inside the marketplace; anything else is Claude's copy.
    let root = fs::canonicalize(&dir).ok()?;
    let at = dir.join(&rel);
    fs::canonicalize(&at)
        .ok()
        .filter(|real| real.starts_with(&root) && real.is_dir())
        .map(|_| at)
}

/// A URL with any user or token taken out, so it never travels on.
fn without_user(url: &str) -> String {
    match url.split_once("://") {
        Some((scheme, rest)) => {
            let (host_and_path, after) = match rest.find('/') {
                Some(i) => (&rest[..i], &rest[i..]),
                None => (rest, ""),
            };
            let host = host_and_path
                .rsplit_once('@')
                .map_or(host_and_path, |(_, h)| h);
            format!("{scheme}://{host}{after}")
        }
        None => url.rsplit_once('@').map_or(url, |(_, h)| h).to_string(),
    }
}

/// A marketplace source's repo: a git URL, or a GitHub `owner/repo`; a
/// directory on this machine is none.
fn repo_of(source: &Value) -> Option<String> {
    match source["source"].as_str()? {
        "git" | "url" => source["url"].as_str().map(without_user),
        "github" => source["repo"]
            .as_str()
            .map(|r| format!("https://github.com/{r}")),
        _ => None,
    }
}

fn registered(servers: &Value) -> Vec<Unit> {
    servers
        .as_object()
        .into_iter()
        .flatten()
        .map(|(name, s)| {
            let mut u = Unit::new(Kind::Mcp, name.clone());
            u.registration = s["command"].as_str().map(|command| Registration {
                command: command.into(),
                args: strings(&s["args"]),
                env: s["env"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter_map(|(k, v)| Some((k.clone(), kept(v.as_str()?))))
                    .collect::<BTreeMap<_, _>>(),
            });
            u
        })
        .collect()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str().map(str::to_string))
        .collect()
}

/// Each `<dir>/<name>/SKILL.md`, in name order.
fn skills(dir: &Path) -> Vec<Unit> {
    let mut out: Vec<Unit> = entries(dir)
        .into_iter()
        .filter(|p| p.join("SKILL.md").is_file())
        .filter_map(|p| {
            let mut u = Unit::new(Kind::Skill, p.file_name()?.to_string_lossy());
            u.manifest = Some(p.join("SKILL.md"));
            Some(u)
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// Each `<dir>/<name>.md`, in name order.
fn agents(dir: &Path) -> Vec<Unit> {
    let mut out: Vec<Unit> = entries(dir)
        .into_iter()
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "md"))
        .filter_map(|p| {
            let mut u = Unit::new(Kind::Agent, p.file_stem()?.to_string_lossy());
            u.manifest = Some(p);
            Some(u)
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn entries(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            !p.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('.'))
        })
        .collect()
}

/// Whether a record parses into the shape this reader takes.
type Shape = fn(&Value) -> bool;

/// An object whose every member is of the shape `inner` takes.
fn object_of(v: &Value, inner: fn(&Value) -> bool) -> bool {
    v.as_object().is_some_and(|m| m.values().all(inner))
}

/// A JSON file as it parses, or none.
fn raw(p: &Path) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(p).ok()?).ok()
}

/// A JSON file, or null when it is absent or not JSON: Claude Code's own
/// files, read leniently since their format is Claude Code's.
fn json(p: &Path) -> Value {
    fs::read_to_string(p)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null)
}

/// The cache's format: a file of another one is read afresh.
const CACHE_FORMAT: u32 = 3;

/// What one read of Claude Code's records gives.
#[derive(Serialize, Deserialize, Default, Clone)]
struct Read {
    units: Vec<Unit>,
    not_understood: Vec<String>,
}

#[derive(Serialize, Deserialize)]
struct Cached {
    format: u32,
    stamps: Vec<(PathBuf, Option<u128>)>,
    read: Read,
}

fn read_cache(at: &Path, stamps: &[(PathBuf, Option<u128>)]) -> Option<Read> {
    let c: Cached = serde_json::from_str(&fs::read_to_string(at).ok()?).ok()?;
    (c.format == CACHE_FORMAT && c.stamps == stamps).then_some(c.read)
}

/// Kept in place of an env value a registration holds as written: a
/// literal can be a secret, which the ghost never holds, so it is not
/// read past here, nor kept in the cache.
pub const NOT_KEPT: &str = "(a literal, not kept)";

/// An env value as the ghost keeps it: a `${VAR}` reference as written.
fn kept(v: &str) -> String {
    match v.starts_with("${") && v.ends_with('}') {
        true => v.into(),
        false => NOT_KEPT.into(),
    }
}

/// The cache, in a file only its owner can read.
fn write_cache(at: &Path, stamps: &[(PathBuf, Option<u128>)], read: &Read) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    let dir = at.parent().unwrap_or(Path::new("/"));
    fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700))?;
    let c = Cached {
        format: CACHE_FORMAT,
        stamps: stamps.to_vec(),
        read: read.clone(),
    };
    let tmp = dir.join(format!(".{}.tmp", std::process::id()));
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)?;
    io::Write::write_all(&mut f, &serde_json::to_vec(&c)?)?;
    fs::rename(tmp, at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn put(p: &Path, v: &Value) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, v.to_string()).unwrap();
    }

    /// Claude Code's records as 2.1.319 writes them after a plugin install
    /// at user scope, a project install, MCP servers at each scope, and a
    /// skill and agent by hand.
    fn shell(t: &TempDir) -> (Claude, PathBuf) {
        let (cfg, proj) = (t.path().join("cfg"), t.path().join("proj"));
        let cache = t.path().join("cache/p/1.0.0");
        put(
            &cache.join(".claude-plugin/plugin.json"),
            &json!({"name": "p"}),
        );
        fs::create_dir_all(cache.join("skills/s")).unwrap();
        fs::write(cache.join("skills/s/SKILL.md"), "---\nname: s\n---\n").unwrap();
        put(
            &cfg.join("plugins/installed_plugins.json"),
            &json!({"version": 2, "plugins": {
                "p@mk": [{"scope": "user", "installPath": cache, "version": "1.0.0",
                          "installedAt": "2026-11-09T23:41:50.943Z"}],
                "q@git": [{"scope": "project", "installPath": t.path().join("q"), "projectPath": proj,
                           "installedAt": "2026-11-01T00:00:00.000Z", "gitCommitSha": "abc"}],
                "r@mk": [{"scope": "project", "installPath": t.path().join("r"), "projectPath": "/elsewhere"}]}}),
        );
        put(
            &cfg.join("plugins/known_marketplaces.json"),
            &json!({"mk": {"source": {"source": "directory", "path": "/m"}},
                    "git": {"source": {"source": "git", "url": "https://example.org/v.git"}}}),
        );
        put(
            &cfg.join("settings.json"),
            &json!({"enabledPlugins": {"p@mk": true, "q@git": true}}),
        );
        put(
            &proj.join(".claude/settings.local.json"),
            &json!({"enabledPlugins": {"q@git": false}}),
        );
        put(
            &cfg.join(".claude.json"),
            &json!({"mcpServers": {"um": {"type": "stdio", "command": "/bin/echo", "args": ["--store", "/s"], "env": {}}},
                    "projects": {proj.display().to_string(): {
                        "mcpServers": {"lm": {"type": "stdio", "command": "/bin/echo", "args": ["a"], "env": {"K": "V"}}},
                        "disabledMcpjsonServers": ["off"]}}}),
        );
        put(
            &proj.join(".mcp.json"),
            &json!({"mcpServers": {"pm": {"type": "stdio", "command": "/bin/true", "args": [], "env": {}},
                                   "off": {"command": "/bin/false"}}}),
        );
        fs::create_dir_all(cfg.join("skills/hand")).unwrap();
        fs::write(cfg.join("skills/hand/SKILL.md"), "---\nname: hand\n---\n").unwrap();
        fs::create_dir_all(cfg.join("agents")).unwrap();
        fs::write(cfg.join("agents/helper.md"), "---\nname: helper\n---\n").unwrap();
        let c = Claude {
            config: Some(ConfigDir::new(&t.path().join("home"), Some(&cfg))),
            cache: None,
            vars: None,
        };
        (c, proj)
    }

    fn summary(units: &[Unit]) -> Vec<String> {
        units
            .iter()
            .map(|u| {
                format!(
                    "{:?} {} {}",
                    u.kind,
                    u.name,
                    if u.enabled { "on" } else { "off" }
                )
            })
            .collect()
    }

    #[test]
    fn every_unit_claude_has_is_read_with_where_and_whether_it_is_on() {
        let t = TempDir::new().unwrap();
        let (c, proj) = shell(&t);
        let units = c.units(Some(&proj)).unwrap();
        assert_eq!(
            summary(&units),
            [
                "Plugin p@mk on",
                "Plugin q@git off",
                "Mcp lm on",
                "Mcp um on",
                "Skill hand on",
                "Agent helper on"
            ]
        );
        let p = &units[0];
        assert_eq!(p.contents.len(), 1);
        assert_eq!(
            p.from.as_ref().unwrap().since.as_deref(),
            Some("2026-11-09")
        );
        assert_eq!(
            p.from.as_ref().unwrap().repo,
            None,
            "a directory marketplace is no repo"
        );
        let q = units[1].from.clone().unwrap();
        assert_eq!(
            (q.repo.as_deref(), q.rev.as_deref()),
            (Some("https://example.org/v.git"), Some("abc"))
        );
        let reg = |n: &str| {
            units
                .iter()
                .find(|u| u.name == n)
                .unwrap()
                .registration
                .clone()
                .unwrap()
        };
        assert_eq!(reg("um").args, ["--store", "/s"]);
        assert_eq!(reg("lm").env["K"], NOT_KEPT, "a literal is not held");
    }

    #[test]
    fn away_from_the_project_only_the_users_units_count() {
        let t = TempDir::new().unwrap();
        let (c, _) = shell(&t);
        let units = c.units(None).unwrap();
        assert_eq!(
            summary(&units),
            [
                "Plugin p@mk on",
                "Mcp um on",
                "Skill hand on",
                "Agent helper on"
            ]
        );
    }

    #[test]
    fn a_second_call_with_unchanged_times_does_not_read_again() {
        let t = TempDir::new().unwrap();
        let (mut c, proj) = shell(&t);
        c.cache = Some(t.path().join("state/shells/claude"));
        let first = c.units(Some(&proj)).unwrap();
        let global = t.path().join("cfg/.claude.json");
        let when = fs::metadata(&global).unwrap().modified().unwrap();
        let mut v = json(&global);
        v["mcpServers"]["new"] = json!({"command": "/bin/new"});
        put(&global, &v);
        fs::File::options()
            .write(true)
            .open(&global)
            .unwrap()
            .set_modified(when)
            .unwrap();
        assert_eq!(c.units(Some(&proj)).unwrap(), first, "read from the cache");
        fs::File::options()
            .write(true)
            .open(&global)
            .unwrap()
            .set_modified(when + std::time::Duration::from_secs(1))
            .unwrap();
        let again = c.units(Some(&proj)).unwrap();
        assert!(
            again.iter().any(|u| u.name == "new"),
            "a new time reads again"
        );
    }

    #[test]
    fn a_directory_marketplaces_plugin_is_read_where_it_lives_and_a_moved_link_is_followed() {
        let t = TempDir::new().unwrap();
        let (mut c, _) = shell(&t);
        c.cache = Some(t.path().join("state"));
        let gen = |n: &str| {
            let m = t.path().join(n);
            put(
                &m.join(".claude-plugin/marketplace.json"),
                &json!({"name": "mk", "plugins": [{"name": "p", "source": "./plugins/p"}]}),
            );
            fs::create_dir_all(m.join("plugins/p/skills").join(n)).unwrap();
            fs::write(
                m.join("plugins/p/skills").join(n).join("SKILL.md"),
                "---\nname: x\n---\n",
            )
            .unwrap();
        };
        gen("one");
        gen("two");
        let link = t.path().join("profile");
        std::os::unix::fs::symlink(t.path().join("one"), &link).unwrap();
        put(
            &t.path().join("cfg/plugins/known_marketplaces.json"),
            &json!({"mk": {"source": {"source": "directory", "path": link}}}),
        );
        let skills = |c: &Claude| {
            let u = c.units(None).unwrap();
            let p = u.iter().find(|u| u.name == "p@mk").unwrap().clone();
            (
                p.dir.unwrap(),
                p.contents
                    .iter()
                    .map(|s| s.name.clone())
                    .collect::<Vec<_>>(),
            )
        };
        assert_eq!(
            skills(&c),
            (link.join("plugins/p"), vec!["one".to_string()])
        );
        fs::remove_file(&link).unwrap();
        std::os::unix::fs::symlink(t.path().join("two"), &link).unwrap();
        assert_eq!(skills(&c).1, ["two"], "the cache follows the link");
    }

    #[test]
    fn the_cache_keeps_no_literal_env_value_and_only_its_owner_reads_it() {
        use std::os::unix::fs::PermissionsExt;
        let t = TempDir::new().unwrap();
        let (mut c, proj) = shell(&t);
        let global = t.path().join("cfg/.claude.json");
        let mut v = json(&global);
        v["mcpServers"]["tok"] =
            json!({"command": "/bin/x", "env": {"TOKEN": "ghp_LITERAL", "REF": "${MY_TOKEN}"}});
        put(&global, &v);
        let cache = t.path().join("state/shells/claude");
        c.cache = Some(cache.clone());
        let first = c.units(Some(&proj)).unwrap();
        let tok = first.iter().find(|u| u.name == "tok").unwrap();
        assert_eq!(tok.registration.as_ref().unwrap().env["TOKEN"], NOT_KEPT);
        let file = fs::read_dir(&cache)
            .unwrap()
            .flatten()
            .next()
            .unwrap()
            .path();
        let text = fs::read_to_string(&file).unwrap();
        assert!(
            !text.contains("ghp_LITERAL") && text.contains("${MY_TOKEN}"),
            "{text}"
        );
        assert_eq!(
            fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&cache).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            c.units(Some(&proj)).unwrap(),
            first,
            "the cache answers the same"
        );
    }

    #[test]
    fn a_name_at_several_scopes_is_the_one_claude_runs_and_a_project_server_needs_approval() {
        let t = TempDir::new().unwrap();
        let (c, proj) = shell(&t);
        let global = t.path().join("cfg/.claude.json");
        let mut v = json(&global);
        v["projects"][proj.display().to_string()]["mcpServers"]["um"] =
            json!({"command": "/bin/local"});
        put(&global, &v);
        let units = c.units(Some(&proj)).unwrap();
        let um: Vec<&Unit> = units.iter().filter(|u| u.name == "um").collect();
        assert_eq!(um.len(), 1);
        assert_eq!(um[0].registration.as_ref().unwrap().command, "/bin/local");
        assert!(
            !units.iter().any(|u| u.name == "pm"),
            "not approved, not run"
        );
        put(
            &proj.join(".claude/settings.json"),
            &json!({"enabledMcpjsonServers": ["pm"], "enableAllProjectMcpServers": true}),
        );
        let units = c.units(Some(&proj)).unwrap();
        assert!(
            !units.iter().any(|u| u.name == "pm"),
            "a repo cannot approve itself"
        );
        put(
            &proj.join(".claude/settings.local.json"),
            &json!({"enabledMcpjsonServers": ["pm"]}),
        );
        let units = c.units(Some(&proj)).unwrap();
        assert!(units.iter().find(|u| u.name == "pm").unwrap().enabled);
    }

    #[test]
    fn a_marketplace_url_loses_its_user_and_token() {
        assert_eq!(
            without_user("https://u:t0k@host.example/r.git"),
            "https://host.example/r.git"
        );
        assert_eq!(
            without_user("https://host.example/a@b"),
            "https://host.example/a@b"
        );
        assert_eq!(
            without_user("git@host.example:me/r.git"),
            "host.example:me/r.git"
        );
    }

    #[test]
    fn a_directory_plugin_outside_its_marketplace_is_read_from_claudes_copy() {
        let t = TempDir::new().unwrap();
        let m = t.path().join("mk");
        put(
            &m.join(".claude-plugin/marketplace.json"),
            &json!({"plugins": [{"name": "p", "source": "../elsewhere"}]}),
        );
        fs::create_dir_all(t.path().join("elsewhere")).unwrap();
        assert_eq!(
            live(&json!({"source": "directory", "path": m}), "p@mk"),
            None
        );
    }

    /// One record changed to a shape this reader does not take: it is
    /// named, and every other record still reads.
    fn changed(file: &str, v: Value) -> (Vec<String>, Vec<Unit>) {
        let t = TempDir::new().unwrap();
        let (mut c, proj) = shell(&t);
        let at = match file {
            ".mcp.json" => proj.join(file),
            f => t.path().join("cfg").join(f),
        };
        let mut g = json(&t.path().join("cfg/.claude.json"));
        g["lastReleaseNotesSeen"] = "9.9.9".into();
        put(&t.path().join("cfg/.claude.json"), &g);
        if file == ".claude.json" {
            let mut v = v;
            v["lastReleaseNotesSeen"] = "9.9.9".into();
            put(&at, &v);
        } else {
            put(&at, &v);
        }
        c.cache = Some(t.path().join("state"));
        let named = c.not_understood(Some(&proj));
        let units = c.units(Some(&proj)).unwrap();
        assert_eq!(
            c.not_understood(Some(&proj)),
            named,
            "the cache says the same"
        );
        (named, units)
    }

    #[test]
    fn a_plugin_registry_of_another_shape_is_named() {
        let (named, units) = changed(
            "plugins/installed_plugins.json",
            json!({"version": 3, "plugins": {}}),
        );
        assert_eq!(named.len(), 1, "{named:?}");
        assert!(
            named[0].ends_with(
                "plugins/installed_plugins.json: shape not understood (Claude Code 9.9.9)"
            ),
            "{named:?}"
        );
        assert!(
            units.iter().any(|u| u.name == "um"),
            "the other records still read"
        );
        let (named, _) = changed(
            "plugins/installed_plugins.json",
            json!({"version": 2, "plugins": {"p@mk": {}}}),
        );
        assert_eq!(
            named.len(),
            1,
            "an entry that is not a list of installs: {named:?}"
        );
    }

    #[test]
    fn a_marketplace_list_of_another_shape_is_named() {
        let (named, _) = changed("plugins/known_marketplaces.json", json!({"mk": "a string"}));
        assert!(
            named[0].contains("known_marketplaces.json: shape not understood"),
            "{named:?}"
        );
    }

    #[test]
    fn a_claude_json_whose_servers_are_not_an_object_is_named() {
        let (named, units) = changed(".claude.json", json!({"mcpServers": ["um"]}));
        assert!(
            named[0].contains(".claude.json: shape not understood (Claude Code 9.9.9)"),
            "{named:?}"
        );
        assert!(
            units.iter().any(|u| u.name == "p@mk"),
            "the plugins still read"
        );
    }

    #[test]
    fn a_projects_mcp_json_of_another_shape_is_named() {
        let (named, _) = changed(".mcp.json", json!({"servers": {}}));
        assert!(
            named[0].contains(".mcp.json: shape not understood"),
            "{named:?}"
        );
    }

    #[test]
    fn records_of_the_shapes_read_name_nothing_and_a_missing_one_is_no_change() {
        let t = TempDir::new().unwrap();
        let (c, proj) = shell(&t);
        assert!(c.not_understood(Some(&proj)).is_empty());
        fs::remove_file(t.path().join("cfg/plugins/known_marketplaces.json")).unwrap();
        assert!(c.not_understood(Some(&proj)).is_empty());
        put(&t.path().join("cfg/.claude.json"), &json!("not an object"));
        let named = c.not_understood(Some(&proj));
        assert!(
            named[0].ends_with("(Claude Code, its version not known)"),
            "{named:?}"
        );
    }

    #[test]
    fn no_config_dir_has_nothing_installed() {
        let c = Claude {
            config: None,
            cache: None,
            vars: None,
        };
        assert!(c.units(None).unwrap().is_empty());
    }
}
