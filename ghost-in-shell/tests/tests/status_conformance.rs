//! The ghost status conformance set (`ghost-in-shell/tests/conformance/
//! <case>/`): each case's Claude Code home and units, and the bring-ins
//! `ghost status` reports for them, each ready, not ready or unreadable.
//! A case's `clis` are stubs on PATH, its `env` the variables set. Paths a
//! case's records name are written `@RUN@`, for the run dir.

use ghost::environment::Host;
use plain::claude_config::ConfigDir;
use serde_json::Value;
use shell_claude::status::bring_ins;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::TempDir;

#[test]
fn every_ghost_status_case() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("conformance");
    let mut cases: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    cases.sort();
    assert!(cases.len() >= 7, "{cases:?}");
    let failed: Vec<String> = cases
        .iter()
        .filter_map(|case| {
            run(case)
                .err()
                .map(|e| format!("{}: {e}", case.file_name().unwrap().to_string_lossy()))
        })
        .collect();
    assert!(failed.is_empty(), "\n{}", failed.join("\n"));
}

fn run(case: &Path) -> Result<(), String> {
    let spec: Value =
        serde_json::from_str(&fs::read_to_string(case.join("case.json")).unwrap()).unwrap();
    let tmp = TempDir::new().unwrap();
    let (home, bin) = (tmp.path().join("home"), tmp.path().join("bin"));
    copy(&case.join("home"), &home);
    copy(&case.join("units"), &tmp.path().join("units"));
    run_dir_in(tmp.path(), tmp.path());
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&bin).unwrap();
    for cli in spec["clis"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, _)| k)
    {
        let at = bin.join(cli);
        fs::write(&at, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&at, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let host = Host {
        path: Some(bin.into_os_string()),
        set: spec["env"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|v| v.as_str().unwrap().to_string())
            .collect(),
    };
    let got = bring_ins(&ConfigDir::new(&home, None), &host).map_err(|e| format!("{e:#}"))?;
    let mut got: Vec<(String, String, String)> = got
        .lines
        .iter()
        .map(|b| {
            (
                b.part.clone(),
                b.name.clone(),
                b.readiness.label().to_string(),
            )
        })
        .collect();
    let mut want: Vec<(String, String, String)> = spec["expect"]["bring_ins"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|b| {
            let s = |k: &str| b[k].as_str().unwrap().to_string();
            (s("part"), s("name"), s("state"))
        })
        .collect();
    got.sort();
    want.sort();
    match got == want {
        true => Ok(()),
        false => Err(format!("bring-ins {got:?}, want {want:?}")),
    }
}

/// Fill `@RUN@` with the run dir in every file under `dir`.
fn run_dir_in(dir: &Path, run: &Path) {
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if fs::symlink_metadata(&p).is_ok_and(|m| m.is_dir()) {
            run_dir_in(&p, run);
        } else if let Ok(text) = fs::read_to_string(&p) {
            if text.contains("@RUN@") {
                fs::write(&p, text.replace("@RUN@", &run.display().to_string())).unwrap();
            }
        }
    }
}

/// Copy a tree; nothing there, nothing copied. The cases hold no links.
fn copy(from: &Path, to: &Path) {
    let Ok(meta) = fs::symlink_metadata(from) else {
        return;
    };
    assert!(!meta.file_type().is_symlink(), "{}", from.display());
    if meta.is_dir() {
        fs::create_dir_all(to).unwrap();
        for e in fs::read_dir(from).unwrap() {
            let e = e.unwrap();
            copy(&e.path(), &to.join(e.file_name()));
        }
    } else {
        fs::copy(from, to).unwrap();
    }
}
