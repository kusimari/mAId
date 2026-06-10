//! Symlink state machine. Manages the `$HOME`-facing symlinks
//! declared by [`crate::registry::REGISTRY`].
//!
//! Plain `std::fs` so the outcome can be a typed enum and the
//! branches are exhaustive `match` over filesystem state. Direct
//! port of the TS `deploy.ts` and its 11 unit tests.

use crate::registry::{Entry, REGISTRY};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct DeployOptions<'a> {
    pub home: &'a Path,
    pub checkout: &'a Path,
    pub dry_run: bool,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeployStatus {
    Created,
    AlreadyOk,
    Replaced,
    SkippedMissingSource,
    SkippedNonSymlink { existing: ExistingKind },
    SkippedWrongSymlink { current_target: PathBuf },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExistingKind {
    File,
    Dir,
    Symlink,
}

#[derive(Debug)]
pub struct DeployResult {
    // Used by tests to identify which registry entry produced a
    // result; main only uses `status` + `target` for output.
    #[allow(dead_code)]
    pub entry: Entry,
    pub status: DeployStatus,
    pub target: PathBuf,
}

pub fn deploy(opts: &DeployOptions) -> io::Result<Vec<DeployResult>> {
    REGISTRY.iter().map(|e| deploy_one(*e, opts)).collect()
}

fn deploy_one(entry: Entry, opts: &DeployOptions) -> io::Result<DeployResult> {
    let target = opts.home.join(entry.home_subpath);
    let source = opts.checkout.join(entry.source_subpath);

    if !path_exists(&source) {
        return Ok(DeployResult {
            entry,
            status: DeployStatus::SkippedMissingSource,
            target,
        });
    }

    if let Some(parent) = target.parent() {
        if !path_exists(parent) && !opts.dry_run {
            fs::create_dir_all(parent)?;
        }
    }

    let lstat = match fs::symlink_metadata(&target) {
        Ok(m) => Some(m),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };

    let Some(meta) = lstat else {
        // Fresh create.
        if !opts.dry_run {
            std::os::unix::fs::symlink(&source, &target)?;
        }
        return Ok(DeployResult {
            entry,
            status: DeployStatus::Created,
            target,
        });
    };

    if meta.file_type().is_symlink() {
        let current = fs::read_link(&target)?;
        if current == source {
            return Ok(DeployResult {
                entry,
                status: DeployStatus::AlreadyOk,
                target,
            });
        }
        if opts.force {
            if !opts.dry_run {
                fs::remove_file(&target)?;
                std::os::unix::fs::symlink(&source, &target)?;
            }
            return Ok(DeployResult {
                entry,
                status: DeployStatus::Replaced,
                target,
            });
        }
        return Ok(DeployResult {
            entry,
            status: DeployStatus::SkippedWrongSymlink {
                current_target: current,
            },
            target,
        });
    }

    // Regular file or real directory — never overwrite.
    let kind = if meta.is_dir() {
        ExistingKind::Dir
    } else {
        ExistingKind::File
    };
    Ok(DeployResult {
        entry,
        status: DeployStatus::SkippedNonSymlink { existing: kind },
        target,
    })
}

// ── undeploy ─────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UndeployStatus {
    NotDeployed,
    Removed { was: PathBuf },
    ForceRemoved { existing: ExistingKind },
    SkippedForeignSymlink { current_target: PathBuf },
    SkippedNonSymlink { existing: ExistingKind },
}

#[derive(Debug)]
pub struct UndeployResult {
    #[allow(dead_code)]
    pub entry: Entry,
    pub status: UndeployStatus,
    pub target: PathBuf,
}

pub fn undeploy(opts: &DeployOptions) -> io::Result<Vec<UndeployResult>> {
    REGISTRY.iter().map(|e| undeploy_one(*e, opts)).collect()
}

fn undeploy_one(entry: Entry, opts: &DeployOptions) -> io::Result<UndeployResult> {
    let target = opts.home.join(entry.home_subpath);
    let expected = opts.checkout.join(entry.source_subpath);

    let lstat = match fs::symlink_metadata(&target) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok(UndeployResult {
                entry,
                status: UndeployStatus::NotDeployed,
                target,
            });
        }
        Err(e) => return Err(e),
    };

    if lstat.file_type().is_symlink() {
        let current = fs::read_link(&target)?;
        if current == expected {
            if !opts.dry_run {
                fs::remove_file(&target)?;
            }
            return Ok(UndeployResult {
                entry,
                status: UndeployStatus::Removed { was: current },
                target,
            });
        }
        if opts.force {
            if !opts.dry_run {
                fs::remove_file(&target)?;
            }
            return Ok(UndeployResult {
                entry,
                status: UndeployStatus::ForceRemoved {
                    existing: ExistingKind::Symlink,
                },
                target,
            });
        }
        return Ok(UndeployResult {
            entry,
            status: UndeployStatus::SkippedForeignSymlink {
                current_target: current,
            },
            target,
        });
    }

    let kind = if lstat.is_dir() {
        ExistingKind::Dir
    } else {
        ExistingKind::File
    };
    if opts.force {
        if !opts.dry_run {
            if lstat.is_dir() {
                fs::remove_dir_all(&target)?;
            } else {
                fs::remove_file(&target)?;
            }
        }
        return Ok(UndeployResult {
            entry,
            status: UndeployStatus::ForceRemoved { existing: kind },
            target,
        });
    }
    Ok(UndeployResult {
        entry,
        status: UndeployStatus::SkippedNonSymlink { existing: kind },
        target,
    })
}

fn path_exists(p: &Path) -> bool {
    fs::symlink_metadata(p).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::REGISTRY;
    use std::fs;
    use tempfile::TempDir;

    fn make_checkout() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("sources/skills")).unwrap();
        fs::create_dir_all(dir.path().join("sources/agents")).unwrap();
        fs::create_dir_all(dir.path().join("sources/commands")).unwrap();
        fs::create_dir_all(dir.path().join("sources/claude")).unwrap();
        fs::create_dir_all(dir.path().join("sources/kiro")).unwrap();
        fs::write(
            dir.path().join("sources/claude/CLAUDE.md"),
            "---\nname: x\ndescription: y\n---\nTop.\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("sources/kiro/KIRO.md"),
            "---\nname: x\ndescription: y\n---\nKiro.\n",
        )
        .unwrap();
        dir
    }

    fn make_home() -> TempDir {
        TempDir::new().unwrap()
    }

    fn opts<'a>(home: &'a Path, checkout: &'a Path) -> DeployOptions<'a> {
        DeployOptions {
            home,
            checkout,
            dry_run: false,
            force: false,
        }
    }

    #[test]
    fn deploy_fresh_home_creates_every_registry_entry() {
        let checkout = make_checkout();
        let home = make_home();
        let results = deploy(&opts(home.path(), checkout.path())).unwrap();

        assert_eq!(results.len(), REGISTRY.len());
        for r in &results {
            assert_eq!(
                r.status,
                DeployStatus::Created,
                "expected created for {}",
                r.entry.home_subpath
            );
            let link = fs::read_link(&r.target).unwrap();
            assert_eq!(link, checkout.path().join(r.entry.source_subpath));
        }
    }

    #[test]
    fn deploy_second_run_is_already_ok() {
        let checkout = make_checkout();
        let home = make_home();
        let o = opts(home.path(), checkout.path());
        deploy(&o).unwrap();
        let second = deploy(&o).unwrap();
        for r in second {
            assert_eq!(r.status, DeployStatus::AlreadyOk);
        }
    }

    #[test]
    fn deploy_wrong_symlink_skipped_without_force() {
        let checkout = make_checkout();
        let home = make_home();
        let first = REGISTRY[0];
        let target = home.path().join(first.home_subpath);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink("/nonexistent/elsewhere.md", &target).unwrap();

        let results = deploy(&opts(home.path(), checkout.path())).unwrap();
        let first_result = results
            .iter()
            .find(|r| r.entry.home_subpath == first.home_subpath)
            .unwrap();
        assert!(matches!(
            first_result.status,
            DeployStatus::SkippedWrongSymlink { .. }
        ));
    }

    #[test]
    fn deploy_wrong_symlink_replaced_with_force() {
        let checkout = make_checkout();
        let home = make_home();
        let first = REGISTRY[0];
        let target = home.path().join(first.home_subpath);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink("/nonexistent/elsewhere.md", &target).unwrap();

        let o = DeployOptions {
            force: true,
            ..opts(home.path(), checkout.path())
        };
        let results = deploy(&o).unwrap();
        let first_result = results
            .iter()
            .find(|r| r.entry.home_subpath == first.home_subpath)
            .unwrap();
        assert_eq!(first_result.status, DeployStatus::Replaced);
        let actual = fs::read_link(&target).unwrap();
        assert_eq!(actual, checkout.path().join(first.source_subpath));
    }

    #[test]
    fn deploy_dry_run_makes_no_changes() {
        let checkout = make_checkout();
        let home = make_home();
        let o = DeployOptions {
            dry_run: true,
            ..opts(home.path(), checkout.path())
        };
        let results = deploy(&o).unwrap();
        for r in results {
            assert_eq!(r.status, DeployStatus::Created);
            assert!(!path_exists(&r.target));
        }
    }

    #[test]
    fn deploy_real_file_not_overwritten() {
        let checkout = make_checkout();
        let home = make_home();
        let first = REGISTRY[0];
        let target = home.path().join(first.home_subpath);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, "user content").unwrap();

        let results = deploy(&opts(home.path(), checkout.path())).unwrap();
        let first_result = results
            .iter()
            .find(|r| r.entry.home_subpath == first.home_subpath)
            .unwrap();
        assert!(matches!(
            first_result.status,
            DeployStatus::SkippedNonSymlink {
                existing: ExistingKind::File
            }
        ));
        let still = fs::read_to_string(&target).unwrap();
        assert_eq!(still, "user content");
    }

    #[test]
    fn undeploy_clean_home_reports_not_deployed() {
        let checkout = make_checkout();
        let home = make_home();
        let results = undeploy(&opts(home.path(), checkout.path())).unwrap();
        assert_eq!(results.len(), REGISTRY.len());
        for r in results {
            assert_eq!(r.status, UndeployStatus::NotDeployed);
        }
    }

    #[test]
    fn undeploy_removes_managed_symlinks() {
        let checkout = make_checkout();
        let home = make_home();
        let o = opts(home.path(), checkout.path());
        deploy(&o).unwrap();

        let results = undeploy(&o).unwrap();
        for r in results {
            assert!(matches!(r.status, UndeployStatus::Removed { .. }));
            assert!(!path_exists(&r.target));
        }
    }

    #[test]
    fn undeploy_idempotent() {
        let checkout = make_checkout();
        let home = make_home();
        let o = opts(home.path(), checkout.path());
        deploy(&o).unwrap();
        undeploy(&o).unwrap();
        let second = undeploy(&o).unwrap();
        for r in second {
            assert_eq!(r.status, UndeployStatus::NotDeployed);
        }
    }

    #[test]
    fn undeploy_foreign_symlink_skipped_without_force() {
        let checkout = make_checkout();
        let home = make_home();
        let first = REGISTRY[0];
        let target = home.path().join(first.home_subpath);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink("/nonexistent/elsewhere.md", &target).unwrap();

        let results = undeploy(&opts(home.path(), checkout.path())).unwrap();
        let first_result = results
            .iter()
            .find(|r| r.entry.home_subpath == first.home_subpath)
            .unwrap();
        assert!(matches!(
            first_result.status,
            UndeployStatus::SkippedForeignSymlink { .. }
        ));
        // Still present after the skip.
        assert!(fs::symlink_metadata(&target)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn undeploy_foreign_symlink_force_removed() {
        let checkout = make_checkout();
        let home = make_home();
        let first = REGISTRY[0];
        let target = home.path().join(first.home_subpath);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink("/nonexistent/elsewhere.md", &target).unwrap();

        let o = DeployOptions {
            force: true,
            ..opts(home.path(), checkout.path())
        };
        let results = undeploy(&o).unwrap();
        let first_result = results
            .iter()
            .find(|r| r.entry.home_subpath == first.home_subpath)
            .unwrap();
        assert_eq!(
            first_result.status,
            UndeployStatus::ForceRemoved {
                existing: ExistingKind::Symlink
            }
        );
        assert!(!path_exists(&target));
    }

    #[test]
    fn undeploy_user_file_preserved_without_force() {
        let checkout = make_checkout();
        let home = make_home();
        let first = REGISTRY[0];
        let target = home.path().join(first.home_subpath);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, "user content").unwrap();

        let results = undeploy(&opts(home.path(), checkout.path())).unwrap();
        let first_result = results
            .iter()
            .find(|r| r.entry.home_subpath == first.home_subpath)
            .unwrap();
        assert!(matches!(
            first_result.status,
            UndeployStatus::SkippedNonSymlink {
                existing: ExistingKind::File
            }
        ));
        let still = fs::read_to_string(&target).unwrap();
        assert_eq!(still, "user content");
    }

    #[test]
    fn undeploy_dry_run_makes_no_changes() {
        let checkout = make_checkout();
        let home = make_home();
        let o = opts(home.path(), checkout.path());
        deploy(&o).unwrap();
        let dry = DeployOptions { dry_run: true, ..o };
        let results = undeploy(&dry).unwrap();
        for r in results {
            assert!(matches!(r.status, UndeployStatus::Removed { .. }));
            // Still present (dry-run).
            assert!(fs::symlink_metadata(&r.target)
                .unwrap()
                .file_type()
                .is_symlink());
        }
    }
}
