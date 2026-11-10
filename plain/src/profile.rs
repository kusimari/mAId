//! Profile: where a package's install lives, so the agents point at it and
//! not at a checkout. With `nix` on PATH it is a nix profile: each install
//! a generation built from the checkout's flake, kept 30 days, and a
//! rollback one generation back. Without nix it is a real dir the package
//! is installed into, with no generations, so no rollback. Each package
//! has its own. See the initiative spec `harness-core`, "Installing and
//! wiring".

use crate::shared::find_on;
use crate::stage;
use anyhow::{anyhow, bail, Context, Result};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One package's profile.
pub struct Profile {
    /// The profile link (nix) or dir.
    pub at: PathBuf,
    /// The flake to build from, and its attribute.
    pub flake: PathBuf,
    pub attr: String,
    /// What a generation must hold for the agents to use it; the first is
    /// what marks a profile dir as an earlier install of the package.
    pub needs: Vec<&'static str>,
    /// The PATH `nix` is found on.
    pub path: Option<OsString>,
}

/// How a profile was installed.
#[derive(Debug, PartialEq, Eq)]
pub enum Installed {
    /// A nix generation, by number.
    Generation(u32),
    /// A real dir, staged by the given step.
    Dir,
}

impl Profile {
    fn nix(&self) -> Option<PathBuf> {
        find_on(self.path.as_deref(), "nix")
    }

    /// Install the package: with nix, build the flake's attribute into a
    /// new generation; without, run `stage` into a fresh dir and put it at
    /// the profile.
    pub fn install(&self, stage: impl FnOnce(&Path) -> Result<()>) -> Result<Installed> {
        if let Some(parent) = self.at.parent() {
            fs::create_dir_all(parent)?;
        }
        let Some(nix) = self.nix() else {
            if fs::symlink_metadata(&self.at).is_ok_and(|m| m.file_type().is_symlink()) {
                bail!(
                    "{} is a nix profile and nix is not on PATH; put nix on PATH, or remove the profile",
                    self.at.display()
                );
            }
            let staged = self.sibling("staged");
            remove(&staged)?;
            stage(&staged)?;
            stage::install(&staged, &self.at, self.needs.first().copied().unwrap_or(""))?;
            remove(&staged)?;
            return Ok(Installed::Dir);
        };
        let flake = format!("{}#{}", self.flake.display(), self.attr);
        let out =
            run(Command::new(&nix).args(["build", "--no-link", "--print-out-paths", &flake]))?;
        self.drop_rolled_back()?;
        let live = fs::read_link(&self.at).ok();
        run(Command::new(&nix)
            .args(["build", "--profile"])
            .arg(&self.at)
            .arg(out.trim()))?;
        // The wipe keeps the newest generation older than the cutoff. With
        // no new generation that is the live one, and the one before it
        // would go.
        if fs::read_link(&self.at).ok() != live {
            run(Command::new(&nix)
                .args(["profile", "wipe-history", "--profile"])
                .arg(&self.at)
                .args(["--older-than", "30d"]))?;
        }
        self.generation()
            .map(Installed::Generation)
            .ok_or_else(|| anyhow!("nix built no generation at {}", self.at.display()))
    }

    /// The generation the profile is on, when it is a nix profile.
    pub fn generation(&self) -> Option<u32> {
        number(&fs::read_link(&self.at).ok()?)
    }

    fn generations(&self) -> Vec<(u32, PathBuf)> {
        let (Some(parent), Some(name)) = (self.at.parent(), self.at.file_name()) else {
            return vec![];
        };
        let prefix = format!("{}-", name.to_string_lossy());
        let mut found: Vec<(u32, PathBuf)> = fs::read_dir(parent)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
            .filter_map(|e| Some((number(Path::new(&e.file_name()))?, e.path())))
            .collect();
        found.sort();
        found
    }

    /// Generations above the live one are installs a rollback returned
    /// from. nix rolls back to the next lower number, so unless they go, a
    /// rollback after the next install lands on them instead.
    fn drop_rolled_back(&self) -> Result<()> {
        let Some(live) = self.generation() else {
            return Ok(());
        };
        for (n, link) in self.generations().into_iter().filter(|(n, _)| *n > live) {
            fs::remove_file(&link)?;
            println!("dropped generation {n} (rolled back from)");
        }
        Ok(())
    }

    /// Switch one generation back. A generation missing what the agents
    /// are pointed at is refused, and nothing changes. The caller points
    /// the agents at it again.
    pub fn rollback(&self) -> Result<u32> {
        let nix = self.nix().ok_or_else(|| {
            anyhow!(
                "rollback needs nix: {} is a dir with no generations",
                self.at.display()
            )
        })?;
        let from = self
            .generation()
            .ok_or_else(|| anyhow!("no install at {} to roll back from", self.at.display()))?;
        let (to, link) = self
            .generations()
            .into_iter()
            .rev()
            .find(|(n, _)| *n < from)
            .ok_or_else(|| anyhow!("no install before generation {from} to roll back to"))?;
        if let Some(need) = self.needs.iter().find(|n| !link.join(n).exists()) {
            bail!("generation {to} has no {need}, so the agents cannot use it; staying on generation {from}");
        }
        run(Command::new(&nix)
            .args(["profile", "rollback", "--profile"])
            .arg(&self.at)
            .args(["--to", &to.to_string()]))?;
        Ok(to)
    }

    /// Remove the profile and every generation. Their store paths go at
    /// the next nix GC.
    pub fn uninstall(&self) -> Result<()> {
        for (_, link) in self.generations() {
            fs::remove_file(link)?;
        }
        remove(&self.at)
    }

    /// One line: the generation and where it points, or that it is a dir,
    /// or missing.
    pub fn status(&self) -> String {
        let at = self.at.display();
        match (self.generation(), fs::canonicalize(&self.at)) {
            (Some(n), Ok(to)) => format!("profile  {at} generation {n} -> {}", to.display()),
            (None, Ok(_)) => format!("profile  {at} (a dir: no generations)"),
            (_, Err(_)) => format!("profile  {at} missing"),
        }
    }

    fn sibling(&self, what: &str) -> PathBuf {
        let name = self.at.file_name().unwrap_or_default().to_string_lossy();
        self.at.with_file_name(format!(".{name}.{what}"))
    }
}

/// The generation number in a `<name>-<n>-link`.
fn number(link: &Path) -> Option<u32> {
    let name = link.file_name()?.to_str()?;
    name.strip_suffix("-link")?.rsplit('-').next()?.parse().ok()
}

fn remove(p: &Path) -> Result<()> {
    match fs::symlink_metadata(p) {
        Ok(m) if m.is_dir() => fs::remove_dir_all(p)?,
        Ok(_) => fs::remove_file(p)?,
        Err(_) => {}
    }
    Ok(())
}

fn run(cmd: &mut Command) -> Result<String> {
    let out = cmd
        .output()
        .with_context(|| format!("cannot run {cmd:?}"))?;
    if !out.status.success() {
        bail!(
            "{cmd:?} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    fn profile(dir: &Path, path: Option<OsString>) -> Profile {
        Profile {
            at: dir.join("state/profile"),
            flake: dir.join("checkout"),
            attr: "default".into(),
            needs: vec!["bin/tool"],
            path,
        }
    }

    #[test]
    fn without_nix_the_profile_is_a_dir_the_stage_is_put_at() {
        let dir = TempDir::new().unwrap();
        let p = profile(dir.path(), Some(dir.path().join("empty").into()));
        let got = p
            .install(|out| {
                fs::create_dir_all(out.join("bin"))?;
                Ok(fs::write(out.join("bin/tool"), "1")?)
            })
            .unwrap();
        assert_eq!(got, Installed::Dir);
        assert_eq!(fs::read_to_string(p.at.join("bin/tool")).unwrap(), "1");
        p.install(|out| {
            fs::create_dir_all(out.join("bin"))?;
            Ok(fs::write(out.join("bin/tool"), "2")?)
        })
        .unwrap();
        assert_eq!(fs::read_to_string(p.at.join("bin/tool")).unwrap(), "2");
        assert!(
            p.status().contains("a dir: no generations"),
            "{}",
            p.status()
        );
        let e = p.rollback().unwrap_err().to_string();
        assert!(e.contains("rollback needs nix"), "{e}");
        p.uninstall().unwrap();
        assert!(p.status().ends_with("missing"));
    }

    #[test]
    fn without_nix_a_nix_profile_is_left_alone() {
        let dir = TempDir::new().unwrap();
        let p = profile(dir.path(), None);
        fs::create_dir_all(dir.path().join("state")).unwrap();
        symlink("profile-1-link", &p.at).unwrap();
        let e = p.install(|_| Ok(())).unwrap_err().to_string();
        assert!(e.contains("nix is not on PATH"), "{e}");
    }

    /// A stub `nix` that echoes. Written by a child shell, so no test
    /// thread holds it open for writing while another forks, which would
    /// leave it busy.
    fn stub_nix(dir: &Path) -> OsString {
        let bin = dir.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let ok = Command::new("sh")
            .arg("-c")
            .arg("printf '#!/bin/sh\\necho /nix/store/out\\n' > \"$1/nix\" && chmod 755 \"$1/nix\"")
            .arg("sh")
            .arg(&bin)
            .status()
            .unwrap();
        assert!(ok.success());
        bin.into_os_string()
    }

    #[test]
    fn a_rollback_refuses_a_generation_missing_what_the_agents_need() {
        let dir = TempDir::new().unwrap();
        let p = profile(dir.path(), Some(stub_nix(dir.path())));
        let state = dir.path().join("state");
        for n in [1, 2] {
            fs::create_dir_all(state.join(format!("gen{n}"))).unwrap();
            symlink(
                state.join(format!("gen{n}")),
                state.join(format!("profile-{n}-link")),
            )
            .unwrap();
        }
        symlink("profile-2-link", &p.at).unwrap();
        let e = p.rollback().unwrap_err().to_string();
        assert!(e.contains("generation 1 has no bin/tool"), "{e}");
        fs::create_dir_all(state.join("gen1/bin")).unwrap();
        fs::write(state.join("gen1/bin/tool"), "").unwrap();
        assert_eq!(p.rollback().unwrap(), 1);
    }

    #[test]
    fn install_drops_the_generations_a_rollback_returned_from() {
        let dir = TempDir::new().unwrap();
        let p = profile(dir.path(), Some(stub_nix(dir.path())));
        let state = dir.path().join("state");
        fs::create_dir_all(&state).unwrap();
        for n in [1, 2, 3] {
            symlink("/nix/store/x", state.join(format!("profile-{n}-link"))).unwrap();
        }
        symlink("profile-1-link", &p.at).unwrap();
        // The stub builds no new generation, so install says so.
        assert!(p.install(|_| unreachable!()).is_ok());
        let there = |n: u32| fs::symlink_metadata(state.join(format!("profile-{n}-link"))).is_ok();
        assert_eq!((there(1), there(2), there(3)), (true, false, false));
    }

    #[test]
    fn a_generation_number_is_read_from_its_link_name() {
        assert_eq!(number(Path::new("/s/profile-12-link")), Some(12));
        assert_eq!(number(Path::new("/s/ghost-profile-3-link")), Some(3));
        assert_eq!(number(Path::new("/s/profile")), None);
    }
}
