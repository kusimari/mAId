//! A part list mAId cannot read stops install and uninstall before they
//! change anything: ownership is unknown. Through the binary, under a temp
//! HOME, profile and parts dir, with logging stub CLIs.

use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

const STUB: &str = "#!/bin/sh\necho \"$0 $*\" >> \"$HOME/../calls\"\necho '[]'\n";

struct Env {
    _tmp: TempDir,
    root: PathBuf,
}

impl Env {
    /// A profile holding the shipped skills, so install has work to do.
    fn new() -> Env {
        let tmp = TempDir::new().unwrap();
        let root = fs::canonicalize(tmp.path()).unwrap();
        for d in ["home/.claude", "parts", "bin", "profile/share/maid"] {
            fs::create_dir_all(root.join(d)).unwrap();
        }
        let skills = Path::new(env!("CARGO_MANIFEST_DIR")).join("../content/skills");
        symlink(
            fs::canonicalize(skills).unwrap(),
            root.join("profile/share/maid/skills"),
        )
        .unwrap();
        for cli in ["claude", "codex"] {
            let p = root.join("bin").join(cli);
            fs::write(&p, STUB).unwrap();
            fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        }
        fs::write(
            root.join("home/.claude/settings.json"),
            "{\"theme\":\"dark\"}\n",
        )
        .unwrap();
        Env { _tmp: tmp, root }
    }
    fn at(&self, p: &str) -> PathBuf {
        self.root.join(p)
    }
    fn run(&self, verb: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_build-tool"))
            .arg(verb)
            .env("HOME", self.at("home"))
            .env("PATH", self.at("bin"))
            .env("MAID_PROFILE", self.at("profile"))
            .env("MY_HARNESS_PARTS", self.at("parts"))
            .env("MAID_MEMORY_STORE", self.at("memory-store"))
            .env_remove("CODEX_HOME")
            .env_remove("CLAUDE_CONFIG_DIR")
            .output()
            .unwrap()
    }
    /// Every path under HOME, the profile and the parts dir, with its bytes
    /// or its link target, plus the stubs' call log.
    fn snapshot(&self) -> Vec<(PathBuf, Vec<u8>)> {
        let mut out = tree(&self.at("home"));
        out.extend(tree(&self.at("profile")));
        out.extend(tree(&self.at("parts")));
        out.push((
            self.at("calls"),
            fs::read(self.at("calls")).unwrap_or_default(),
        ));
        out
    }
}

fn tree(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = vec![];
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            let meta = fs::symlink_metadata(&p).unwrap();
            if meta.file_type().is_symlink() {
                let to = fs::read_link(&p).unwrap();
                out.push((p, to.into_os_string().into_encoded_bytes()));
            } else if meta.is_dir() {
                out.push((p.clone(), b"dir".to_vec()));
                stack.push(p);
            } else {
                out.push((p.clone(), fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn an_unreadable_or_unknown_part_list_stops_install_and_uninstall() {
    let e = Env::new();
    let list = e.at("parts/maid-harness.json");
    // What the list holds; none: it is a dir.
    let bad = [
        (
            "format 2",
            Some(r#"{"format":2,"part":"maid-harness","harnesses":{}}"#),
        ),
        ("not JSON", Some("not json")),
        ("a dir", None),
    ];
    for (what, text) in bad {
        match text {
            Some(t) => fs::write(&list, t).unwrap(),
            None => fs::create_dir(&list).unwrap(),
        }
        for verb in ["install", "uninstall"] {
            let before = e.snapshot();
            let out = e.run(verb);
            let text = String::from_utf8_lossy(&out.stderr);
            assert_eq!(out.status.code(), Some(1), "{verb} with {what}: {text}");
            assert!(
                text.contains("maid-harness.json"),
                "{verb} with {what}: {text}"
            );
            assert_eq!(e.snapshot(), before, "{verb} with {what} changed something");
        }
        match text {
            Some(_) => fs::remove_file(&list).unwrap(),
            None => fs::remove_dir(&list).unwrap(),
        }
    }

    // The same run with no list does act, so the checks above can fail.
    let before = e.snapshot();
    e.run("install");
    assert_ne!(e.snapshot(), before, "install with no list changed nothing");
}
