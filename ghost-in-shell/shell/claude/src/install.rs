//! The ghost's install: it validates its declarations, builds its own
//! profile, reuses plain's install or runs it from this checkout, then
//! places its plugin with Claude Code's own commands and writes its own
//! install record. The record is the installer's, for its uninstall and
//! status; the ghost's sessions never read it. See the initiative spec
//! `harness-core`, "Plain, then the ghost".

use crate::installed::Claude;
use crate::package::{self, MARKETPLACE, MARKETPLACE_DIR, PLUGIN_ID};
use crate::plugins::{place_steps, Cli};
use crate::reuse::{self, Reuse};
use anyhow::{anyhow, bail, Result};
use plain::claude_config::ConfigDir;
use plain::profile::Profile;
use plain::record::{self, Item, Own, Package, What};
use plain::shared::Agent;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// What the ghost says of mAId's plugin turned off.
pub const DISABLED: &str = "mAId's plugin is disabled; its skills are off";

/// The ghost's install record's package name.
pub const RECORD: &str = "ghost";
/// The value on its plugin item: the version of mAId's plugin it was
/// installed with.
pub const WITH_PLAIN: &str = "plain";
/// What a generation of the ghost's profile must hold for Claude Code.
pub const NEEDS: &[&str] = &[
    "bin/ghost-claude",
    "share/ghost/marketplace/plugins/ghost/.claude-plugin/plugin.json",
];

/// Where the ghost's install reads and writes, from the environment.
pub struct Setup {
    /// The checkout it installs from.
    pub root: PathBuf,
    pub profile: PathBuf,
    pub claude: ConfigDir,
    /// The install-records folder.
    pub records: PathBuf,
    /// The ghost's own state.
    pub state: PathBuf,
    /// This executable, staged into a profile without nix.
    pub exe: PathBuf,
    pub path: Option<OsString>,
}

impl Setup {
    pub fn from_env(root: &Path) -> Result<Setup> {
        let var = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let home = var("HOME").ok_or_else(|| anyhow!("HOME is not set"))?;
        let state_home = var("XDG_STATE_HOME").unwrap_or_else(|| format!("{home}/.local/state"));
        let path_or = |k: &str, default: String| -> Result<PathBuf> {
            let p = PathBuf::from(var(k).unwrap_or(default));
            p.is_absolute()
                .then_some(p)
                .ok_or_else(|| anyhow!("{k} must be an absolute path"))
        };
        Ok(Setup {
            root: root.to_path_buf(),
            profile: path_or("GHOST_PROFILE", format!("{state_home}/maid/ghost/profile"))?,
            claude: ConfigDir::from_env()?.ok_or_else(|| anyhow!("HOME is not set"))?,
            records: path_or("INSTALL_RECORDS", format!("{state_home}/install-records"))?,
            state: ghost::config::Config::from_env()?.state,
            exe: std::env::current_exe()?,
            path: std::env::var_os("PATH"),
        })
    }

    pub fn resources(&self) -> PathBuf {
        self.root.join(plain::shared::CONTENT_DIR)
    }

    pub fn plugin(&self) -> PathBuf {
        self.root.join("ghost-in-shell/shell/claude/plugin")
    }

    pub fn profile(&self) -> Profile {
        Profile {
            at: self.profile.clone(),
            flake: self.root.clone(),
            attr: "ghost".into(),
            needs: NEEDS.to_vec(),
            path: self.path.clone(),
        }
    }

    pub fn record(&self) -> Own {
        Own {
            records: self.records.clone(),
            package: RECORD.into(),
        }
    }

    pub fn installed(&self) -> Claude {
        Claude {
            config: Some(self.claude.clone()),
            cache: None,
            vars: None,
        }
    }

    pub fn cli(&self) -> Cli {
        Cli {
            config: self.claude.clone(),
            path: self.path.clone(),
        }
    }
}

/// Every declaration the ghost will read, and its own skill's standard.
pub fn validate(s: &Setup) -> Result<()> {
    crate::validate::cmd(&[s.resources()])?;
    validate_plugin(s)
}

/// The ghost's own plugin: its declarations, and its skill's standard.
pub fn validate_plugin(s: &Setup) -> Result<()> {
    crate::validate::cmd(&[s.plugin()])?;
    plain::validate::check_content(&[s.plugin()])
        .map_err(|e| anyhow!("the ghost's plugin is not valid:\n{}", e.join("\n")))?;
    Ok(())
}

/// The version of mAId's plugin plain's install would put in place from
/// this checkout: the revision the reuse check wants. With nix that is the
/// flake's package, which sees tracked files only; without, the working
/// tree staged as plain stages it.
pub fn this_checkouts_plain(s: &Setup) -> Result<String> {
    let Some(nix) = plain::shared::find_on(s.path.as_deref(), "nix") else {
        let dir = tempfile::TempDir::new()?;
        return plain::stage::plain(&s.resources(), &dir.path().join("out"), &[]);
    };
    let out = std::process::Command::new(nix)
        .args(["build", "--no-link", "--print-out-paths"])
        .arg(format!("{}#default", s.root.display()))
        .output()?;
    if !out.status.success() {
        bail!(
            "cannot build this checkout's plain package: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let built = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    let manifest = built
        .join(plain::stage::PLUGIN)
        .join(".claude-plugin/plugin.json");
    let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&manifest)?)?;
    v["version"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow!("{} names no version", manifest.display()))
}

/// Where plain's install keeps its things must be known before anything is
/// built, so a run outside `just` fails before the ghost's profile moves.
fn plain_env() -> Result<()> {
    plain::shared::profile_dir()?;
    plain::shared::manifest_dirs()?;
    plain::shared::memory_store_file()?;
    plain::shared::config_dir()?;
    Ok(())
}

/// `ghost-claude install`.
pub fn install(s: &Setup) -> Result<u8> {
    validate(s)?;
    plain_env()?;
    foreign_marketplace(s)?;
    s.profile()
        .install(|out| package::stage(&s.plugin(), &s.exe, out, &s.profile).map(drop))?;
    println!("{}", s.profile().status());
    let want = this_checkouts_plain(s)?;
    let mut rc = 0;
    let plain_at = match reuse::check(&s.installed(), &want)? {
        Reuse::Reused(v) => {
            println!("plain    reused: mAId's plugin is this checkout's ({v})");
            v
        }
        Reuse::Disabled(v) => {
            println!("plain    {DISABLED}; reused as it is, not re-enabled ({v})");
            v
        }
        Reuse::Install(why) => {
            println!("plain    {why}; installing the resources from this checkout");
            rc = plain::install::all(&s.root, None)?;
            // What is live now, not what was wanted, so the record and the
            // warning name what Claude Code has.
            reuse::plain_plugin(&s.installed())?
                .and_then(|u| reuse::version_of(&u))
                .unwrap_or(want)
        }
    };
    place(s)?;
    write_record(s, &plain_at)?;
    Ok(rc)
}

/// Bring Claude Code to the ghost's plugin in its profile, and read back
/// that it is listed at the profile's version.
pub fn place(s: &Setup) -> Result<()> {
    let want = package::version_in(&s.profile)
        .ok_or_else(|| anyhow!("no ghost plugin in {}", s.profile.display()))?;
    let cli = s.cli();
    let dir = s.profile.join(MARKETPLACE_DIR);
    foreign_marketplace(s)?;
    let registered = cli.marketplace(MARKETPLACE)?;
    let steps = place_steps(
        MARKETPLACE,
        PLUGIN_ID,
        &dir,
        registered.as_deref(),
        cli.listed(PLUGIN_ID)?.as_ref(),
        &want,
    );
    for step in &steps {
        let args: Vec<&str> = step.iter().map(String::as_str).collect();
        cli.run(&args)?;
    }
    match cli.listed(PLUGIN_ID)? {
        Some(l) if l.version == want => {
            let on = if l.enabled { "on" } else { "off" };
            println!("claude   plugin {PLUGIN_ID} {want} ({on})");
            Ok(())
        }
        other => bail!("claude plugin {PLUGIN_ID} is {other:?} after install, want {want}"),
    }
}

/// Refuse a marketplace named `ghost-in-shell` that someone else added.
fn foreign_marketplace(s: &Setup) -> Result<()> {
    match s.cli().marketplace(MARKETPLACE)? {
        Some(at) if !is_ghosts(&at) => bail!(
            "a marketplace named {MARKETPLACE} is at {}, which is not a ghost profile's; remove it (claude plugin marketplace remove {MARKETPLACE}) to install the ghost",
            at.display()
        ),
        _ => Ok(()),
    }
}

/// Whether a marketplace dir is a ghost profile's, by its shape, so a
/// marketplace of that name that someone else added is never taken over.
pub fn is_ghosts(at: &Path) -> bool {
    at.ends_with(MARKETPLACE_DIR)
}

/// The ghost's own record: its marketplace and plugin (with the version of
/// mAId's plugin it was installed with), its profile, this checkout.
pub fn write_record(s: &Setup, plain_at: &str) -> Result<()> {
    let own = s.record();
    let mut plugin = Item::placed(What::Plugin, PLUGIN_ID);
    plugin.at = Some(s.profile.join(package::PLUGIN_DIR));
    plugin.values.insert(WITH_PLAIN.into(), plain_at.into());
    record::record(
        &own,
        Agent::Claude,
        &[What::Marketplace, What::Plugin],
        None,
        vec![Item::placed(What::Marketplace, MARKETPLACE), plugin],
    )?;
    record::record_package(
        &own,
        Package {
            source: Some(plain::install::project_source(
                plain::install::Checkout::read(&s.root),
            )),
            root: Some(s.profile.clone()),
            config: None,
        },
    )
}

/// The version of mAId's plugin the record says the ghost was installed
/// with.
pub fn installed_with(s: &Setup) -> Result<Option<String>> {
    Ok(record::placed(&s.record(), Agent::Claude)?
        .into_iter()
        .find(|i| i.kind == What::Plugin && i.name == PLUGIN_ID)
        .and_then(|i| i.values.get(WITH_PLAIN).cloned()))
}

/// The warning when the live mAId plugin is not the one the ghost was
/// installed with: it never refuses, and sessions keep working.
pub fn revision_warning(s: &Setup) -> Result<Option<String>> {
    let live = reuse::plain_plugin(&s.installed())?.and_then(|u| reuse::version_of(&u));
    let with = installed_with(s)?;
    Ok(match (live, with) {
        (Some(live), Some(with)) if live != with => Some(format!(
            "warning  plain at {live}, ghost at {with}; `just ghost install` installs both from this checkout"
        )),
        (None, Some(_)) => Some(
            "warning  mAId's plugin is not installed; `just install` or `just ghost install` puts it back".into(),
        ),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup(dir: &Path) -> Setup {
        Setup {
            root: dir.join("checkout"),
            profile: dir.join("ghost/profile"),
            claude: ConfigDir::new(&dir.join("home"), None),
            records: dir.join("records"),
            state: dir.join("state"),
            exe: dir.join("exe"),
            path: None,
        }
    }

    #[test]
    fn the_record_is_the_ghosts_own_and_names_plains_version() {
        let dir = TempDir::new().unwrap();
        let s = setup(dir.path());
        write_record(&s, "1.0.0-a").unwrap();
        assert!(s.records.join("ghost.json").is_file());
        assert!(
            !s.records.join("maid.json").exists(),
            "plain's is not touched"
        );
        assert_eq!(installed_with(&s).unwrap().as_deref(), Some("1.0.0-a"));
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(s.records.join("ghost.json")).unwrap())
                .unwrap();
        assert_eq!(v["package"], "ghost");
        assert_eq!(v["root"], json_path(&s.profile));
    }

    fn json_path(p: &Path) -> serde_json::Value {
        p.display().to_string().into()
    }

    #[test]
    fn only_a_ghost_profiles_marketplace_is_the_ghosts() {
        assert!(is_ghosts(Path::new(
            "/s/maid/ghost/profile/share/ghost/marketplace"
        )));
        assert!(!is_ghosts(Path::new("/home/me/my-own-marketplace")));
    }

    #[test]
    fn a_mismatch_warns_and_a_match_says_nothing() {
        let dir = TempDir::new().unwrap();
        let s = setup(dir.path());
        assert_eq!(
            revision_warning(&s).unwrap(),
            None,
            "no record, nothing to compare"
        );
        write_record(&s, "1.0.0-a").unwrap();
        let w = revision_warning(&s).unwrap().unwrap();
        assert!(w.contains("mAId's plugin is not installed"), "{w}");
        // mAId's plugin, installed at another version.
        let home = dir.path().join("home");
        let plugin = dir.path().join("maid-plugin");
        std::fs::create_dir_all(plugin.join(".claude-plugin")).unwrap();
        let manifest = |v: &str| {
            std::fs::write(
                plugin.join(".claude-plugin/plugin.json"),
                format!(r#"{{"name":"maid","version":"{v}"}}"#),
            )
            .unwrap()
        };
        manifest("1.0.0-b");
        std::fs::create_dir_all(home.join(".claude/plugins")).unwrap();
        std::fs::write(
            home.join(".claude/plugins/installed_plugins.json"),
            serde_json::json!({"version": 2, "plugins": {"maid@maid": [
                {"scope": "user", "installPath": plugin}]}})
            .to_string(),
        )
        .unwrap();
        std::fs::write(
            home.join(".claude/settings.json"),
            r#"{"enabledPlugins":{"maid@maid":true}}"#,
        )
        .unwrap();
        let w = revision_warning(&s).unwrap().unwrap();
        assert!(w.contains("plain at 1.0.0-b, ghost at 1.0.0-a"), "{w}");
        manifest("1.0.0-a");
        assert_eq!(revision_warning(&s).unwrap(), None);
    }
}
