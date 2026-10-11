//! The whole-status conformance set (`tests/conformance/<case>/`, format in
//! project.md "My harness"): each case's states through `whole::check`, its
//! exit code through the binary. One test, so no other thread forks while a
//! stub is open for writing.

use build_tool::shared::{find_on, Agent};
use build_tool::whole::{check, Found};
use serde_json::Value;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

type Row = (String, String, String, String, Option<String>, String);

#[test]
fn every_conformance_case() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/conformance");
    // Stubs run with a PATH of stubs only, so they name `cat` absolutely.
    let cat = find_on(std::env::var_os("PATH").as_deref(), "cat").expect("cat on PATH");
    let mut cases: Vec<PathBuf> = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    cases.sort();
    assert!(cases.len() > 20, "{cases:?}");
    // A passing test's output is hidden, so a skip fails unless asked for.
    let skips_ok = std::env::var_os("CONFORMANCE_ALLOW_SKIPS").is_some();
    let mut failed = vec![];
    for case in &cases {
        let name = case.file_name().unwrap().to_string_lossy().into_owned();
        match run(case, &cat) {
            Ok(None) => {}
            Ok(Some(skip)) if skips_ok => eprintln!("skipped {name}: {skip}"),
            Ok(Some(skip)) => failed.push(format!(
                "{name}: cannot run here ({skip}); CONFORMANCE_ALLOW_SKIPS=1 skips it"
            )),
            Err(why) => failed.push(format!("{name}: {why}")),
        }
    }
    assert!(failed.is_empty(), "\n{}", failed.join("\n"));
}

/// Run one case: `Ok(Some(why))` when it cannot run here.
fn run(case: &Path, cat: &Path) -> Result<Option<String>, String> {
    let spec: Value = serde_json::from_str(&fs::read_to_string(case.join("case.json")).unwrap())
        .map_err(|e| format!("case.json: {e}"))?;
    let tmp = TempDir::new().unwrap();
    let (parts, home, bin) = (
        tmp.path().join("parts"),
        tmp.path().join("home"),
        tmp.path().join("bin"),
    );
    copy(&case.join("parts"), &parts, case);
    copy(&case.join("home"), &home, case);
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&bin).unwrap();
    for (cli, answers) in spec["clis"].as_object().into_iter().flatten() {
        stub(case, tmp.path(), cli, answers, cat);
    }
    let modes: Vec<(PathBuf, u32)> = spec["modes"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(p, m)| {
            let mode = u32::from_str_radix(m.as_str().unwrap(), 8).unwrap();
            (tmp.path().join(p), mode)
        })
        .collect();
    let _restore = Restore(modes.iter().map(|(p, _)| p.clone()).collect());
    for (p, mode) in &modes {
        fs::set_permissions(p, fs::Permissions::from_mode(*mode)).unwrap();
    }
    let still_readable = modes
        .iter()
        .any(|(p, m)| m & 0o444 == 0 && (fs::read_dir(p).is_ok() || fs::read(p).is_ok()));
    match still_readable {
        true => Ok(Some(
            "a path without read permission can still be read (root?)".into(),
        )),
        false => compare(&spec, &parts, &home, &bin),
    }
}

/// Gives back permission on drop, panic or not, so the temp dir can go.
struct Restore(Vec<PathBuf>);

impl Drop for Restore {
    fn drop(&mut self) {
        for p in self.0.iter().rev() {
            let back = if p.is_dir() { 0o755 } else { 0o644 };
            let _ = fs::set_permissions(p, fs::Permissions::from_mode(back));
        }
    }
}

fn compare(spec: &Value, parts: &Path, home: &Path, bin: &Path) -> Result<Option<String>, String> {
    let expect = &spec["expect"];
    let harness = spec["harness"].as_str();
    let agent = harness.map(|h| *Agent::ALL.iter().find(|a| a.cli() == h).unwrap());
    let mut errors = vec![];
    // The run's working dir is the stub dir, so an empty or relative PATH
    // entry would find a stub there.
    let path: OsString = match spec["path"].as_array() {
        Some(entries) => std::env::join_paths(entries.iter().map(|e| e.as_str().unwrap())).unwrap(),
        None => bin.as_os_str().to_owned(),
    };
    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(bin).unwrap();
    let checked = check(parts, home, Some(path.clone()), agent);
    std::env::set_current_dir(cwd).unwrap();

    match checked {
        Err(e) if expect["aborted"] != true => errors.push(format!("aborted: {e:#}")),
        Err(_) => {}
        Ok(_) if expect["aborted"] == true => errors.push("did not abort".into()),
        Ok(whole) => {
            let mut got: Vec<Row> = whole
                .lines
                .iter()
                .map(|l| {
                    let e = &l.entry;
                    let (h, p) = (l.harness.clone(), l.part.clone());
                    (
                        h,
                        p,
                        e.kind.clone(),
                        e.name.clone(),
                        e.scope.clone(),
                        state(&l.found).into(),
                    )
                })
                .collect();
            let mut want: Vec<Row> = expect["entries"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| {
                    let s = |k: &str| e[k].as_str().unwrap().to_string();
                    let scope = e["scope"].as_str().map(str::to_string);
                    (
                        s("harness"),
                        s("part"),
                        s("kind"),
                        s("name"),
                        scope,
                        s("state"),
                    )
                })
                .collect();
            got.sort();
            want.sort();
            if got != want {
                for w in want.iter().filter(|w| !got.contains(w)) {
                    let like: Vec<_> = got
                        .iter()
                        .filter(|g| (&g.0, &g.3, &g.4) == (&w.0, &w.3, &w.4))
                        .collect();
                    errors.push(format!("want {w:?}, got {like:?}"));
                }
                for g in got.iter().filter(|g| !want.contains(g)) {
                    errors.push(format!("unexpected {g:?}"));
                }
                if errors.is_empty() {
                    errors.push(format!("same rows, different counts: {got:?}"));
                }
            }
            let mut refused: Vec<&str> = whole.refused.iter().map(|r| r.file.as_str()).collect();
            refused.sort();
            let mut want_refused: Vec<&str> = expect["refused"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap())
                .collect();
            want_refused.sort();
            if refused != want_refused {
                errors.push(format!("refused {refused:?}, want {want_refused:?}"));
            }
        }
    }

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_build-tool"));
    cmd.arg("whole")
        .current_dir(bin)
        .env_clear()
        .env("HOME", home)
        .env("MY_HARNESS_PARTS", parts)
        .env("PATH", &path)
        .env("CARGO_MANIFEST_DIR", env!("CARGO_MANIFEST_DIR"));
    if let Some(a) = agent {
        cmd.args(["--agent", a.name()]);
    }
    let out = cmd.output().unwrap();
    if expect["aborted"] == true && !out.stdout.is_empty() {
        errors.push("an abort printed a report".into());
    }
    let want_exit = expect["exit"].as_i64().unwrap();
    if out.status.code() != Some(want_exit as i32) {
        errors.push(format!(
            "exit {:?}, want {want_exit}; stderr: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    match errors.is_empty() {
        true => Ok(None),
        false => Err(errors.join("\n    ")),
    }
}

fn state(found: &Found) -> &'static str {
    match found {
        Found::Present => "present",
        Found::Disabled => "present (disabled)",
        Found::Missing => "missing",
        Found::Skipped(_) => "skipped",
        Found::NotCheckable(_) => "not checkable",
        Found::Unreadable(_) => "unreadable",
    }
}

/// A stub CLI that answers each exact argument string from the case's data
/// and fails with exit 3 for any other.
fn stub(case: &Path, tmp: &Path, cli: &str, answers: &Value, cat: &Path) {
    let data = tmp.join("stub").join(cli);
    fs::create_dir_all(&data).unwrap();
    let mut body = String::from("#!/bin/sh\ncase \"$*\" in\n");
    for (i, (args, a)) in answers.as_object().unwrap().iter().enumerate() {
        assert!(
            !args.contains('\''),
            "{cli} {args:?}: no ' in a stub's arguments"
        );
        let bytes = |text: &str, file: &str| match (a[text].as_str(), a[file].as_str()) {
            (_, Some(f)) => fs::read(case.join(f)).unwrap(),
            (Some(t), None) => t.as_bytes().to_vec(),
            (None, None) => vec![],
        };
        let (out, err) = (data.join(format!("{i}.out")), data.join(format!("{i}.err")));
        fs::write(&out, bytes("stdout", "stdout_file")).unwrap();
        fs::write(&err, bytes("stderr", "stderr_file")).unwrap();
        body += &format!(
            "  '{args}') '{c}' '{o}'; '{c}' '{e}' >&2; exit {x} ;;\n",
            c = cat.display(),
            o = out.display(),
            e = err.display(),
            x = a["exit"].as_i64().unwrap_or(0),
        );
    }
    body += "  *) echo \"stub: $*\" >&2; exit 3 ;;\nesac\n";
    let at = tmp.join("bin").join(cli);
    fs::write(&at, body).unwrap();
    fs::set_permissions(&at, fs::Permissions::from_mode(0o755)).unwrap();
}

/// Copy a file or tree, keeping symlinks as links. Nothing there, nothing
/// copied.
fn copy(from: &Path, to: &Path, root: &Path) {
    let Ok(meta) = fs::symlink_metadata(from) else {
        return;
    };
    if meta.file_type().is_symlink() {
        let target = fs::read_link(from).unwrap();
        // A case's links stay inside its own tree, so a run never reads
        // real files.
        assert!(
            inside(from, &target, root),
            "{} -> {}",
            from.display(),
            target.display()
        );
        std::os::unix::fs::symlink(target, to).unwrap();
    } else if meta.is_dir() {
        fs::create_dir_all(to).unwrap();
        for e in fs::read_dir(from).unwrap() {
            let e = e.unwrap();
            copy(&e.path(), &to.join(e.file_name()), root);
        }
    } else {
        fs::copy(from, to).unwrap();
    }
}

/// Whether a relative link at `link` to `target` stays under `root`.
fn inside(link: &Path, target: &Path, root: &Path) -> bool {
    let mut at = link.parent().unwrap().to_path_buf();
    for c in target.components() {
        match c {
            Component::ParentDir => {
                at.pop();
            }
            Component::Normal(n) => at.push(n),
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    at.starts_with(root)
}
