//! The reuse check: whether Claude Code has mAId's own units, from this
//! checkout. The ghost reuses everything plain installed, so `ghost
//! install` looks at what Claude Code has (through the adapter), never at
//! an installer's record: mAId's plugin `maid@maid` at the user's scope, at
//! the version this checkout's resources stage to. Its content-hash version
//! is the revision. A plugin the person turned off is reused as it is:
//! plain would only leave it off again. See the initiative spec
//! `harness-core`, "Plain, then the ghost".

use ghost::installed::{Installed, Kind, Unit};
use std::fmt;

/// mAId's plugin, as plain installs it: its id, and its marketplace.
pub const PLAIN: &str = "maid@maid";
const MARKETPLACE: &str = "maid";

/// The check's answer.
#[derive(Debug, PartialEq, Eq)]
pub enum Reuse {
    /// mAId's plugin is there, enabled, at this checkout's version.
    Reused(String),
    /// mAId's plugin is there and turned off: reused, never re-enabled.
    Disabled(String),
    /// Plain must be installed from this checkout, and why.
    Install(Why),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Why {
    NotInstalled,
    Other { found: String, want: String },
    NotUnderstood(Vec<String>),
}

impl fmt::Display for Why {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Why::NotInstalled => write!(f, "mAId's plugin is not installed"),
            Why::Other { found, want } => {
                write!(f, "mAId's plugin is at {found}, this checkout's is {want}")
            }
            Why::NotUnderstood(n) => {
                write!(
                    f,
                    "Claude Code's records are not understood: {}",
                    n.join("; ")
                )
            }
        }
    }
}

/// mAId's plugin in what the shell has at the user's scope.
pub fn plain_plugin(installed: &dyn Installed) -> anyhow::Result<Option<Unit>> {
    Ok(installed.units(None)?.into_iter().find(|u| {
        u.kind == Kind::Plugin
            && u.name == PLAIN
            && u.from
                .as_ref()
                .is_some_and(|o| o.marketplace == MARKETPLACE)
    }))
}

/// The version a plugin unit's manifest names.
pub fn version_of(unit: &Unit) -> Option<String> {
    let text = std::fs::read_to_string(unit.manifest.as_ref()?).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v["version"].as_str().map(str::to_string)
}

/// Whether plain is reused, against `want`, this checkout's version.
pub fn check(installed: &dyn Installed, want: &str) -> anyhow::Result<Reuse> {
    let shapes = installed.not_understood(None);
    if !shapes.is_empty() {
        return Ok(Reuse::Install(Why::NotUnderstood(shapes)));
    }
    let Some(unit) = plain_plugin(installed)? else {
        return Ok(Reuse::Install(Why::NotInstalled));
    };
    if !unit.enabled {
        return Ok(Reuse::Disabled(
            version_of(&unit).unwrap_or_else(|| "no version".into()),
        ));
    }
    match version_of(&unit) {
        Some(v) if v == want => Ok(Reuse::Reused(v)),
        found => Ok(Reuse::Install(Why::Other {
            found: found.unwrap_or_else(|| "no version".into()),
            want: want.into(),
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ghost::installed::Origin;
    use std::path::Path;
    use tempfile::TempDir;

    /// A shell with mAId's plugin at `version`, or with records it cannot
    /// read.
    struct Shell(Vec<Unit>, Vec<String>);

    impl Installed for Shell {
        fn units(&self, _: Option<&Path>) -> anyhow::Result<Vec<Unit>> {
            Ok(self.0.clone())
        }
        fn not_understood(&self, _: Option<&Path>) -> Vec<String> {
            self.1.clone()
        }
    }

    fn maid(dir: &TempDir, version: &str, enabled: bool) -> Unit {
        let m = dir.path().join(format!("{version}.json"));
        std::fs::write(&m, format!(r#"{{"name":"maid","version":"{version}"}}"#)).unwrap();
        Unit {
            manifest: Some(m),
            enabled,
            from: Some(Origin {
                marketplace: "maid".into(),
                ..Origin::default()
            }),
            ..Unit::new(Kind::Plugin, "maid@maid")
        }
    }

    #[test]
    fn plain_is_reused_only_when_its_plugin_is_this_checkouts_and_on() {
        let dir = TempDir::new().unwrap();
        let at = |units, shapes| check(&Shell(units, shapes), "1.0.0-a").unwrap();
        assert_eq!(
            at(vec![maid(&dir, "1.0.0-a", true)], vec![]),
            Reuse::Reused("1.0.0-a".into())
        );
        assert_eq!(at(vec![], vec![]), Reuse::Install(Why::NotInstalled));
        assert_eq!(
            at(vec![maid(&dir, "1.0.0-a", false)], vec![]),
            Reuse::Disabled("1.0.0-a".into())
        );
        assert_eq!(
            at(vec![maid(&dir, "1.0.0-b", false)], vec![]),
            Reuse::Disabled("1.0.0-b".into()),
            "a plugin turned off is reused whatever its version, never reinstalled"
        );
        assert_eq!(
            at(vec![maid(&dir, "1.0.0-b", true)], vec![]),
            Reuse::Install(Why::Other {
                found: "1.0.0-b".into(),
                want: "1.0.0-a".into()
            })
        );
        assert!(matches!(
            at(
                vec![maid(&dir, "1.0.0-a", true)],
                vec!["x: shape not understood".into()]
            ),
            Reuse::Install(Why::NotUnderstood(_))
        ));
    }

    #[test]
    fn a_plugin_named_maid_from_another_marketplace_is_not_maids() {
        let dir = TempDir::new().unwrap();
        let mut other = maid(&dir, "1.0.0-a", true);
        other.from = Some(Origin {
            marketplace: "elsewhere".into(),
            ..Origin::default()
        });
        assert_eq!(
            check(&Shell(vec![other], vec![]), "1.0.0-a").unwrap(),
            Reuse::Install(Why::NotInstalled)
        );
    }
}
