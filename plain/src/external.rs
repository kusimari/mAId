//! External pieces: what plain installs from another repo, each pinned,
//! so plain's record names where it came from. The vendor marketplace
//! mAId registers at its pinned commit, and the memory server, pensieve,
//! at the flake's lock. The ghost's package names none today. A unit's own
//! `repository` declaration still decides where a proposal about it goes.
//! See the initiative spec `harness-core`, "Installing and wiring".

use crate::record::Repo;
use std::fs;
use std::path::Path;

/// A vendor's plugin marketplace mAId registers with each plugin agent
/// through its own CLI, and the plugins it installs from it. The content
/// stays the vendor's: nothing of it is copied into the profile.
#[derive(Debug, PartialEq, Eq)]
pub struct Vendor {
    /// The name both CLIs list the marketplace under.
    pub marketplace: &'static str,
    /// A public git URL.
    pub source: &'static str,
    /// The commit codex is pinned to. claude's CLI takes only a branch or
    /// tag, so it holds the commit current when it was first registered.
    pub rev: &'static str,
    pub plugins: &'static [&'static str],
    /// Commands the plugins' skills run, which the environment provides;
    /// status names any not on PATH.
    pub needs: &'static [&'static str],
}

impl Vendor {
    /// A plugin's id as both CLIs list it.
    pub fn plugin_id(&self, plugin: &str) -> String {
        format!("{plugin}@{}", self.marketplace)
    }
}

/// The vendor marketplaces `just install` brings in.
pub const VENDORS: &[Vendor] = &[Vendor {
    marketplace: "anthropic-agent-skills",
    source: "https://github.com/anthropics/skills.git",
    rev: "dbd4588f9e1033efb41dad4bef2f7947c8993d44",
    plugins: &["document-skills"],
    needs: &[
        "python",
        "node",
        "soffice",
        "pandoc",
        "markitdown",
        "qpdf",
        "pdftotext",
        "pdftoppm",
        "pdfimages",
        "magick",
    ],
}];

/// The memory package as the `flake.lock` in `dir` pins it.
pub fn pensieve(dir: &Path) -> Option<Repo> {
    pinned(
        &fs::read_to_string(dir.join("flake.lock")).ok()?,
        "pensieve",
    )
}

/// An input `flake.lock` pins on GitHub, as a project source. Any other pin
/// (a path on this machine, say) gives none, so no local path is recorded.
pub fn pinned(lock: &str, input: &str) -> Option<Repo> {
    let lock: serde_json::Value = serde_json::from_str(lock).ok()?;
    // The root's inputs name each input's node, which may differ from it.
    let root = lock["root"].as_str().unwrap_or("root");
    let node = lock["nodes"][root]["inputs"][input]
        .as_str()
        .unwrap_or(input);
    let locked = &lock["nodes"][node]["locked"];
    let field = |k: &str| locked[k].as_str().filter(|v| !v.is_empty());
    if field("type")? != "github" {
        return None;
    }
    Some(Repo {
        repo: Some(format!(
            "https://github.com/{}/{}",
            field("owner")?,
            field("repo")?
        )),
        rev: Some(field("rev")?.to_string()),
        path: None,
    })
}

/// Where the profile's pensieve came from, for the record. Nix put it there
/// (it resolves into the store): the flake's pin. Found on PATH (a profile
/// dir without nix): that path, and the version it reports, else "version
/// unknown"; never the pin, which nix did not provide. Not there: none.
pub fn pensieve_source(profile: &Path, pin: Option<Repo>) -> Option<Repo> {
    let found = fs::canonicalize(profile.join("bin/pensieve")).ok()?;
    if found.starts_with("/nix/store") {
        return pin;
    }
    Some(Repo {
        repo: None,
        rev: Some(reported_version(&found).unwrap_or_else(|| "version unknown".into())),
        path: Some(found),
    })
}

/// The first line a program prints for `--version`, given two seconds and
/// no input, so one that would serve instead is stopped.
fn reported_version(program: &Path) -> Option<String> {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let mut child = Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    let status = loop {
        match child.try_wait().ok()? {
            Some(s) => break s,
            None if start.elapsed() > Duration::from_secs(2) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    let mut out = String::new();
    std::io::Read::read_to_string(&mut child.stdout.take()?, &mut out).ok()?;
    let line = out.lines().next()?.trim().to_string();
    (status.success() && !line.is_empty()).then_some(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn github_pin() -> Repo {
        Repo {
            repo: Some("https://github.com/me/pensieve".into()),
            rev: Some("p1".into()),
            path: None,
        }
    }

    /// Spelled out: a row edited to the wrong URL or a short commit would
    /// otherwise register something else, or fail only at install.
    #[test]
    fn the_vendors_are_the_vendors_document_skills_pinned_to_a_commit() {
        let [b] = VENDORS else {
            panic!("one vendor plugin expected")
        };
        assert_eq!(b.marketplace, "anthropic-agent-skills");
        assert_eq!(b.source, "https://github.com/anthropics/skills.git");
        assert_eq!(b.plugins, ["document-skills"]);
        assert_eq!(
            b.plugin_id("document-skills"),
            "document-skills@anthropic-agent-skills"
        );
        assert!(
            b.rev.len() == 40 && b.rev.bytes().all(|c| c.is_ascii_hexdigit()),
            "{}",
            b.rev
        );
        assert!(b.needs.contains(&"soffice") && b.needs.contains(&"python"));
    }

    #[test]
    fn the_lock_gives_a_github_pin_as_its_source_and_nothing_else() {
        let lock = |locked: serde_json::Value| {
            serde_json::json!({"nodes": {"pensieve": {"locked": locked}}}).to_string()
        };
        let github = lock(serde_json::json!(
            {"type": "github", "owner": "me", "repo": "pensieve", "rev": "p1"}
        ));
        assert_eq!(pinned(&github, "pensieve"), Some(github_pin()));
        let path = lock(serde_json::json!({"type": "path", "path": "/home/me/pensieve"}));
        assert_eq!(
            pinned(&path, "pensieve"),
            None,
            "a local path is never recorded"
        );
        assert_eq!(pinned(&github, "other"), None);
        assert_eq!(pinned("not json", "pensieve"), None);
        let renamed = serde_json::json!({"root": "root", "nodes": {
            "root": {"inputs": {"pensieve": "pensieve_2"}},
            "pensieve": {"locked": {"type": "path", "path": "/elsewhere"}},
            "pensieve_2": {"locked":
                {"type": "github", "owner": "me", "repo": "pensieve", "rev": "p1"}}}});
        assert_eq!(
            pinned(&renamed.to_string(), "pensieve"),
            Some(github_pin()),
            "the root's inputs name the node"
        );
    }

    #[test]
    fn the_checkouts_lock_is_read_from_its_dir() {
        let dir = TempDir::new().unwrap();
        assert_eq!(pensieve(dir.path()), None);
        let lock = serde_json::json!({"nodes": {"pensieve": {"locked":
            {"type": "github", "owner": "me", "repo": "pensieve", "rev": "p1"}}}});
        fs::write(dir.path().join("flake.lock"), lock.to_string()).unwrap();
        assert_eq!(pensieve(dir.path()), Some(github_pin()));
    }

    #[test]
    fn this_repo_pins_the_memory_package_at_its_published_commit() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let lock = fs::read_to_string(root.join("flake.lock")).unwrap();
        let got = pinned(&lock, "pensieve").expect("pensieve pinned on GitHub");
        assert_eq!(
            got.repo.as_deref(),
            Some("https://github.com/kusimari/pensieve")
        );
        let rev = got.rev.unwrap();
        assert!(
            rev.len() == 40 && rev.chars().all(|c| c.is_ascii_hexdigit()),
            "{rev}"
        );
    }

    /// A program written by a child shell, so no test thread holds it open
    /// for writing while another forks.
    fn program(at: &Path, body: &str) {
        let ok = std::process::Command::new("sh")
            .arg("-c")
            .arg("printf '%s' \"$2\" > \"$1\" && chmod 755 \"$1\"")
            .arg("sh")
            .arg(at)
            .arg(body)
            .status()
            .unwrap();
        assert!(ok.success());
    }

    #[test]
    fn a_pensieve_found_on_path_is_recorded_as_found_never_as_the_pin() {
        let dir = TempDir::new().unwrap();
        let profile = dir.path().join("profile");
        fs::create_dir_all(profile.join("bin")).unwrap();
        assert_eq!(pensieve_source(&profile, Some(github_pin())), None);
        let found = dir.path().join("pensieve");
        program(&found, "#!/bin/sh\necho 'pensieve 1.2.3'\n");
        std::os::unix::fs::symlink(&found, profile.join("bin/pensieve")).unwrap();
        let got = pensieve_source(&profile, Some(github_pin())).unwrap();
        assert_eq!(got.repo, None, "the flake's pin is not what was installed");
        assert_eq!(got.rev.as_deref(), Some("pensieve 1.2.3"));
        assert_eq!(got.path, Some(fs::canonicalize(&found).unwrap()));
        let quiet = dir.path().join("quiet");
        program(&quiet, "#!/bin/sh\nexit 2\n");
        fs::remove_file(profile.join("bin/pensieve")).unwrap();
        std::os::unix::fs::symlink(&quiet, profile.join("bin/pensieve")).unwrap();
        let got = pensieve_source(&profile, Some(github_pin())).unwrap();
        assert_eq!(got.rev.as_deref(), Some("version unknown"));
    }

    #[test]
    fn a_pensieve_nix_put_in_the_profile_is_recorded_as_the_pin() {
        let Some(store) = std::env::var_os("MAID_TEST_PENSIEVE") else {
            return; // only the dev shell has one in the store
        };
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("bin")).unwrap();
        std::os::unix::fs::symlink(store, dir.path().join("bin/pensieve")).unwrap();
        assert_eq!(
            pensieve_source(dir.path(), Some(github_pin())),
            Some(github_pin())
        );
    }
}
