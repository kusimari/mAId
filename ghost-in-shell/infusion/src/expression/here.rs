//! Where a session is: its folder, the repo root above it, the branch, and
//! the files under the root a glob names. Read from the file system only,
//! with no command run, and bounded, since it runs on every prompt.

use globset::{Glob, GlobMatcher};
use std::fs;
use std::path::{Component, Path, PathBuf};

/// How deep and how wide a walk for a glob goes.
const DEPTH: usize = 4;
const ENTRIES: usize = 2_000;

#[derive(Debug, Clone, Default)]
pub struct Here {
    pub cwd: Option<PathBuf>,
    /// The nearest folder at or above `cwd` holding `.git`, else `cwd`.
    pub root: Option<PathBuf>,
    pub branch: Option<String>,
}

impl Here {
    pub fn at(cwd: Option<&Path>) -> Here {
        let Some(cwd) = cwd.filter(|c| c.is_absolute()) else {
            return Here::default();
        };
        let repo = cwd.ancestors().find(|a| a.join(".git").exists());
        Here {
            cwd: Some(cwd.to_path_buf()),
            root: Some(repo.unwrap_or(cwd).to_path_buf()),
            branch: repo.and_then(branch),
        }
    }

    /// The files under the root that `pattern` matches, relative to it, in
    /// a stable order. A pattern that is absolute or goes through `..` is
    /// refused.
    pub fn glob(&self, pattern: &str) -> Result<Vec<PathBuf>, String> {
        let rel = Path::new(pattern);
        if rel.is_absolute() || rel.components().any(|c| c == Component::ParentDir) {
            return Err(format!("{pattern:?} must stay under the location"));
        }
        let Some(root) = &self.root else {
            return Ok(vec![]);
        };
        let m = matcher(pattern)?;
        // Walk only from the pattern's literal folders.
        let prefix: PathBuf = rel
            .components()
            .take_while(|c| {
                !c.as_os_str()
                    .to_string_lossy()
                    .contains(['*', '?', '[', '{'])
            })
            .collect();
        let start = match prefix == rel {
            true => prefix.parent().map(Path::to_path_buf).unwrap_or_default(),
            false => prefix,
        };
        // The walk follows no link, and neither may the folders it starts
        // from: a linked `specs/` would lead outside the location.
        let mut at = root.clone();
        for c in start.components() {
            at.push(c);
            if !fs::symlink_metadata(&at).is_ok_and(|m| m.is_dir()) {
                return Ok(vec![]);
            }
        }
        let mut found = vec![];
        let mut seen = 0;
        walk(root, &start, 0, &mut seen, &mut |p| {
            if m.is_match(p) {
                found.push(p.to_path_buf());
            }
        });
        found.sort();
        Ok(found)
    }
}

pub fn matcher(pattern: &str) -> Result<GlobMatcher, String> {
    globset::GlobBuilder::new(pattern)
        .literal_separator(true)
        .build()
        .map(|g: Glob| g.compile_matcher())
        .map_err(|e| format!("{pattern:?} is not a glob: {e}"))
}

fn walk(root: &Path, rel: &Path, depth: usize, seen: &mut usize, f: &mut impl FnMut(&Path)) {
    if depth > DEPTH {
        return;
    }
    let Ok(rd) = fs::read_dir(root.join(rel)) else {
        return;
    };
    // Bounded before sorting, so one huge folder is not read in full.
    let mut names: Vec<_> = rd
        .flatten()
        .take(ENTRIES.saturating_sub(*seen) + 1)
        .collect();
    names.sort_by_key(|e| e.file_name());
    for e in names {
        *seen += 1;
        if *seen > ENTRIES {
            return;
        }
        let name = e.file_name();
        // Dot entries (`.git`, tool caches) are skipped; a pattern that
        // names a dot folder (`.kdevkit/…`) still starts its walk there.
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let p = rel.join(&name);
        // A link is neither a dir nor a file here: the walk follows none.
        match e.file_type() {
            Ok(t) if t.is_dir() => walk(root, &p, depth + 1, seen, f),
            Ok(t) if t.is_file() => f(&p),
            _ => {}
        }
    }
}

/// The checked-out branch, from `.git/HEAD`, or from the gitdir a
/// worktree's `.git` file names.
fn branch(repo: &Path) -> Option<String> {
    let dot = repo.join(".git");
    let dir = match fs::metadata(&dot).ok()?.is_dir() {
        true => dot,
        false => {
            let text = read_head(&dot)?;
            let at = PathBuf::from(text.strip_prefix("gitdir:")?.trim());
            match at.is_absolute() {
                true => at,
                false => repo.join(at),
            }
        }
    };
    let head = read_head(&dir.join("HEAD"))?;
    head.trim()
        .strip_prefix("ref: refs/heads/")
        .map(str::to_string)
}

/// A `.git` file or a `HEAD` is one short line; read no more than a little.
fn read_head(path: &Path) -> Option<String> {
    use std::io::Read;
    let mut text = String::new();
    fs::File::open(path)
        .ok()?
        .take(4096)
        .read_to_string(&mut text)
        .ok()?;
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn touch(root: &Path, rel: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, "").unwrap();
    }

    #[test]
    fn the_root_is_the_repo_above_the_folder_and_the_branch_is_read() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path().join("repo");
        touch(&repo, ".git/HEAD");
        fs::write(repo.join(".git/HEAD"), "ref: refs/heads/feat/x/work\n").unwrap();
        fs::create_dir_all(repo.join("a/b")).unwrap();
        let here = Here::at(Some(&repo.join("a/b")));
        assert_eq!(here.root.as_deref(), Some(repo.as_path()));
        assert_eq!(here.branch.as_deref(), Some("feat/x/work"));
        let wt = dir.path().join("wt");
        fs::create_dir_all(&wt).unwrap();
        fs::write(
            wt.join(".git"),
            format!("gitdir: {}\n", repo.join(".git").display()),
        )
        .unwrap();
        assert_eq!(Here::at(Some(&wt)).branch.as_deref(), Some("feat/x/work"));
        let bare = Here::at(Some(&dir.path().join("elsewhere")));
        assert_eq!(
            bare.root.as_deref(),
            Some(dir.path().join("elsewhere").as_path())
        );
        assert_eq!(bare.branch, None);
        assert!(Here::at(Some(Path::new("relative"))).root.is_none());
    }

    #[test]
    fn a_glob_finds_files_under_the_root_and_never_leaves_it() {
        let dir = TempDir::new().unwrap();
        for f in [
            "specs/feature/a.md",
            "specs/feature/b.md",
            "specs/project.md",
            "x.xlsx",
            "deep/1/2/3/4/5/far.xlsx",
            ".hidden/y.xlsx",
        ] {
            touch(dir.path(), f);
        }
        let here = Here::at(Some(dir.path()));
        let names = |p: &str| -> Vec<String> {
            here.glob(p)
                .unwrap()
                .iter()
                .map(|x| x.display().to_string())
                .collect()
        };
        assert_eq!(
            names("specs/feature/*.md"),
            ["specs/feature/a.md", "specs/feature/b.md"]
        );
        assert_eq!(names("specs/project.md"), ["specs/project.md"]);
        assert_eq!(
            names("*.xlsx"),
            ["x.xlsx"],
            "one folder deep only, no dot-dirs"
        );
        assert_eq!(names("**/*.xlsx"), ["x.xlsx"], "the walk is bounded");
        let outside = dir.path().join("outside-specs");
        touch(&outside, "feature/secret.md");
        let linked = TempDir::new().unwrap();
        touch(linked.path(), "x.md");
        std::os::unix::fs::symlink(&outside, linked.path().join("specs")).unwrap();
        std::os::unix::fs::symlink(
            outside.join("feature/secret.md"),
            linked.path().join("y.md"),
        )
        .unwrap();
        let there = Here::at(Some(linked.path()));
        assert!(
            there.glob("specs/feature/*.md").unwrap().is_empty(),
            "a linked folder is not walked"
        );
        assert!(there.glob("specs/feature/secret.md").unwrap().is_empty());
        assert_eq!(
            there.glob("*.md").unwrap(),
            [PathBuf::from("x.md")],
            "nor a linked file"
        );
        assert!(here.glob("../*.md").is_err());
        assert!(here.glob("/etc/*").is_err());
    }
}
