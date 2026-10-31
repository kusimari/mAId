//! The work's own record: where a turn bring-in says the work keeps its
//! goal, next step and state (contract field `record`), found under the
//! session's location. The record stays the work's; this only reads it.

use crate::contract;
use crate::here::Here;
use serde::Serialize;
use std::path::PathBuf;

/// At most this many lines of a record's section are read.
const LINES: usize = 30;

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Record {
    pub file: PathBuf,
    pub goal: Option<String>,
    pub state: Option<String>,
    pub next: Option<String>,
}

/// The first of `pointers` (`<file glob>#<heading slug>`) that names a
/// record here: the one file a glob matches, or among several the one
/// whose name is a part of the branch name; none when that cannot choose.
pub fn find(pointers: &[String], here: &Here) -> Result<Option<Record>, String> {
    for pointer in pointers {
        let (glob, anchor) = pointer
            .split_once('#')
            .filter(|(g, a)| !g.is_empty() && !a.is_empty())
            .ok_or_else(|| format!("record {pointer:?} is not <file glob>#<anchor>"))?;
        let files = here.glob(glob)?;
        let Some(rel) = choose(&files, here.branch.as_deref()) else {
            continue;
        };
        let file = here
            .root
            .as_ref()
            .expect("a glob matched under a root")
            .join(rel);
        let section = contract::section(&file, anchor)?;
        let lines: Vec<&str> = section.lines().take(LINES).collect();
        let field =
            |names: &[&str]| -> Option<String> { lines.iter().find_map(|l| labelled(l, names)) };
        let text = contract::read_small(&file).unwrap_or_default();
        return Ok(Some(Record {
            goal: title(&text),
            state: field(&["stage", "state"]),
            next: field(&["ready for", "next"]),
            file,
        }));
    }
    Ok(None)
}

/// The file's first `# ` heading outside fenced code.
fn title(text: &str) -> Option<String> {
    let mut fenced = false;
    for line in text.lines() {
        let t = line.trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
        } else if !fenced {
            if let Some(title) = t.strip_prefix("# ") {
                return Some(title.trim().to_string());
            }
        }
    }
    None
}

/// The one file, or among several the one whose stem is a whole
/// `/`-separated part of the branch name (`hooks` for `feat/hooks/work`).
fn choose<'a>(files: &'a [PathBuf], branch: Option<&str>) -> Option<&'a PathBuf> {
    if let [one] = files {
        return Some(one);
    }
    let parts: Vec<&str> = branch?.split('/').collect();
    let mut hits = files.iter().filter(|f| {
        f.file_stem()
            .and_then(|s| s.to_str())
            .is_some_and(|s| parts.contains(&s))
    });
    match (hits.next(), hits.next()) {
        (Some(f), None) => Some(f),
        _ => None,
    }
}

/// `- **Stage:** dev`, `Stage: dev` or `**Next**: x`: the value, when the
/// label is one of `names`.
fn labelled(line: &str, names: &[&str]) -> Option<String> {
    let l = line.trim_start().trim_start_matches(['-', '*', ' ']);
    let (label, value) = l.split_once(':')?;
    let label = label.trim_matches(['*', ' ']).to_lowercase();
    let value = value.trim().trim_start_matches(['*', ' ']).trim();
    (names.contains(&label.as_str()) && !value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    const SPEC: &str = "```\n# not the title\n```\n# Feature: hooks\n\n## Handoff\n\n- **Stage:** dev\n- **Ready for:** review, once the gates pass\n\n## Next part\n\n- **Stage:** not this one\n";

    fn repo(branch: &str, files: &[&str]) -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(".git")).unwrap();
        fs::write(
            dir.path().join(".git/HEAD"),
            format!("ref: refs/heads/{branch}\n"),
        )
        .unwrap();
        for f in files {
            let p = dir.path().join(f);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, SPEC).unwrap();
        }
        dir
    }

    fn pointers(p: &[&str]) -> Vec<String> {
        p.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn the_record_gives_goal_state_and_next_from_its_section() {
        let dir = repo("main", &["docs/specs/feature/hooks.md"]);
        let here = Here::at(Some(dir.path()));
        let r = find(
            &pointers(&[
                "specs/feature/*.md#handoff",
                "docs/specs/feature/*.md#handoff",
            ]),
            &here,
        )
        .unwrap()
        .unwrap();
        assert_eq!(r.file, dir.path().join("docs/specs/feature/hooks.md"));
        assert_eq!(r.goal.as_deref(), Some("Feature: hooks"));
        assert_eq!(r.state.as_deref(), Some("dev"), "only the named section");
        assert_eq!(r.next.as_deref(), Some("review, once the gates pass"));
    }

    #[test]
    fn among_several_files_the_branch_chooses_or_nothing_does() {
        let files = ["specs/feature/hooks.md", "specs/feature/other.md"];
        let p = pointers(&["specs/feature/*.md#handoff"]);
        let dir = repo("feat/hooks/work", &files);
        let r = find(&p, &Here::at(Some(dir.path()))).unwrap().unwrap();
        assert!(r.file.ends_with("hooks.md"));
        let dir = repo("main", &files);
        assert_eq!(find(&p, &Here::at(Some(dir.path()))).unwrap(), None);
    }

    #[test]
    fn a_pointer_that_leaves_the_location_or_is_not_a_pointer_is_refused() {
        let dir = repo("main", &["a.md"]);
        let here = Here::at(Some(dir.path()));
        for bad in ["../*.md#handoff", "/etc/*.md#x", "a.md", "#x"] {
            assert!(find(&pointers(&[bad]), &here).is_err(), "{bad}");
        }
        assert_eq!(find(&pointers(&["none/*.md#x"]), &here).unwrap(), None);
        assert_eq!(
            find(
                &pointers(&["a.md#handoff"]),
                &Here::at(Some(Path::new("/nonexistent-x")))
            )
            .unwrap(),
            None
        );
    }
}
