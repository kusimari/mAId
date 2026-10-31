//! Harvest: copy each harness's own memory into a store dir, leaving the
//! originals. See project.md "My harness", Memory.
//!
//! Originals are only read. A file whose bytes already sit at `<stem>.md`
//! or `<stem>-N.md` is skipped, else it lands at the first free name, so a
//! rerun changes nothing.

use crate::settings::write_atomic;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// What Claude loads of `MEMORY.md` at session start.
const INDEX_LINES: usize = 200;
const INDEX_BYTES: usize = 25 * 1024;

use maid_core::claude::project_dir_name;

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
    pub projects: Vec<String>,
    pub notes: Vec<String>,
}

impl Harvest {
    pub fn lines(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .projects
            .iter()
            .map(|p| format!("project  {p}"))
            .collect();
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

/// Claude's per-project memory dirs, only those of projects under `project`
/// when given. The name is lossy (`a-b` and `a/b` look alike), so a prefix
/// may take a sibling too; harvest prints every project it takes.
fn claude_candidates(
    home: &Path,
    target: &Path,
    project: Option<&str>,
    report: &mut Harvest,
) -> Result<Vec<Candidate>> {
    let projects = home.join(".claude/projects");
    let target = fs::canonicalize(target).ok();
    let prefix = project.map(project_dir_name);
    let mut dirs: Vec<PathBuf> = match fs::read_dir(&projects) {
        Ok(e) => e
            .filter_map(|e| e.ok())
            .filter(|e| {
                prefix
                    .as_ref()
                    .is_none_or(|p| e.file_name().to_string_lossy().starts_with(p.as_str()))
            })
            .map(|e| e.path().join("memory"))
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
        if let Some(name) = dir.parent().and_then(|p| p.file_name()) {
            report.projects.push(name.to_string_lossy().into_owned());
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

/// Copy every harness's native memory into `<dir>/memory/`. With a
/// `project` prefix, only Claude's projects under it, and nothing of
/// codex, which keeps no projects. `dir` must exist; it is never created.
pub fn harvest(
    home: &Path,
    codex_home: &Path,
    dir: &Path,
    project: Option<&str>,
) -> Result<Harvest> {
    if !dir.is_dir() {
        bail!(
            "{} is not a directory; clone the store first",
            dir.display()
        );
    }
    let target = dir.join("memory");
    let mut report = Harvest::default();
    if !target.is_dir() {
        fs::create_dir_all(&target)?;
        report.notes.push(format!("created  {}", target.display()));
    }
    let mut candidates = claude_candidates(home, &target, project, &mut report)?;
    if project.is_none() {
        candidates.extend(codex_candidates(codex_home)?);
    }

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
    use std::collections::BTreeMap;
    use tempfile::TempDir;

    struct Env {
        _tmp: TempDir,
        home: PathBuf,
        stores: PathBuf,
    }

    fn env() -> Env {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        for d in ["home", "stores/personal"] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        Env {
            home: root.join("home"),
            stores: fs::canonicalize(root.join("stores")).unwrap(),
            _tmp: tmp,
        }
    }

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
        let r = harvest(
            &e.home,
            &e.home.join(".codex"),
            &e.stores.join("personal"),
            None,
        )
        .unwrap();
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
        harvest(
            &e.home,
            &e.home.join(".codex"),
            &e.stores.join("personal"),
            None,
        )
        .unwrap();
        let first = snapshot(&e.stores);
        let r = harvest(
            &e.home,
            &e.home.join(".codex"),
            &e.stores.join("personal"),
            None,
        )
        .unwrap();
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
        let r = harvest(
            &e.home,
            &e.home.join(".codex"),
            &e.stores.join("personal"),
            None,
        )
        .unwrap();
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
        harvest(
            &e.home,
            &e.home.join(".codex"),
            &e.stores.join("personal"),
            None,
        )
        .unwrap();
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
        let r = harvest(
            &e.home,
            &e.home.join(".codex"),
            &e.stores.join("personal"),
            None,
        )
        .unwrap();
        assert!(r.copied.is_empty() && r.present.is_empty());
    }

    #[test]
    fn codex_memories_are_wrapped_as_reference_memories() {
        let e = env();
        let codex = e.home.join(".codex/memories");
        fs::create_dir_all(&codex).unwrap();
        fs::write(codex.join("MEMORY.md"), "# Task Group\n- fact\n").unwrap();
        fs::write(codex.join("raw_memories.md"), "queue").unwrap();
        let r = harvest(
            &e.home,
            &e.home.join(".codex"),
            &e.stores.join("personal"),
            None,
        )
        .unwrap();
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
        harvest(
            &e.home,
            &e.home.join(".codex"),
            &e.stores.join("personal"),
            None,
        )
        .unwrap();
        let mem = e.stores.join("personal/memory");
        fs::remove_file(mem.join(INDEX)).unwrap();
        let r = harvest(
            &e.home,
            &e.home.join(".codex"),
            &e.stores.join("personal"),
            None,
        )
        .unwrap();
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
        harvest(
            &e.home,
            &e.home.join(".codex"),
            &e.stores.join("personal"),
            None,
        )
        .unwrap();
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
        assert!(harvest(
            &e.home,
            &e.home.join(".codex"),
            &e.stores.join("personal"),
            None
        )
        .is_err());
        assert_eq!(fs::read(mem.join(INDEX)).unwrap(), [0xff, 0xfe, b'\n']);
    }

    #[test]
    fn a_long_index_is_warned_about() {
        let e = env();
        let mem = e.stores.join("personal/memory");
        fs::create_dir_all(&mem).unwrap();
        fs::write(mem.join(INDEX), "- line\n".repeat(INDEX_LINES)).unwrap();
        seed_project(&e, "-p1", &[("a.md", &memory("a", "A"))], None);
        let r = harvest(
            &e.home,
            &e.home.join(".codex"),
            &e.stores.join("personal"),
            None,
        )
        .unwrap();
        assert!(r.notes.iter().any(|n| n.starts_with("warning")));
    }

    #[test]
    fn a_project_prefix_takes_only_its_projects_and_no_codex() {
        let e = env();
        seed_project(&e, "-work-api", &[("a.md", &memory("a", "A"))], None);
        seed_project(&e, "-work-api-v2", &[("b.md", &memory("b", "B"))], None);
        seed_project(&e, "-home-garden", &[("c.md", &memory("c", "C"))], None);
        let codex = e.home.join(".codex/memories");
        fs::create_dir_all(&codex).unwrap();
        fs::write(codex.join("MEMORY.md"), "- fact\n").unwrap();
        let r = harvest(
            &e.home,
            &e.home.join(".codex"),
            &e.stores.join("personal"),
            Some("/work/api"),
        )
        .unwrap();
        let mut taken = r.projects.clone();
        taken.sort();
        assert_eq!(taken, ["-work-api", "-work-api-v2"]);
        let mem = e.stores.join("personal/memory");
        assert!(mem.join("a.md").exists() && mem.join("b.md").exists());
        assert!(!mem.join("c.md").exists() && !mem.join("codex-memory.md").exists());
        assert!(r.lines().contains(&"project  -work-api".to_string()));
    }

    #[test]
    fn a_prefix_is_encoded_the_way_claude_names_project_dirs() {
        assert_eq!(project_dir_name("/a/b.c_d e"), "-a-b-c-d-e");
    }

    #[test]
    fn a_store_dir_that_is_not_there_is_refused_and_not_created() {
        let e = env();
        let gone = e.stores.join("gone");
        let err = harvest(&e.home, &e.home.join(".codex"), &gone, None).unwrap_err();
        assert!(err.to_string().contains("clone the store first"), "{err}");
        assert!(!gone.exists());
    }
}
