//! Memory from outside: the `memory` verbs through the binary, under a
//! temp HOME and temp config and state dirs.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

struct Env {
    tmp: TempDir,
}

impl Env {
    fn new() -> Env {
        let tmp = TempDir::new().unwrap();
        for d in ["home", "config", "state", "stores/personal", "stores/work"] {
            fs::create_dir_all(tmp.path().join(d)).unwrap();
        }
        Env { tmp }
    }
    fn at(&self, p: &str) -> PathBuf {
        fs::canonicalize(self.tmp.path()).unwrap().join(p)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_build-tool"))
            .args(args)
            .env("HOME", self.at("home"))
            .env("MY_HARNESS_MEMORY", self.at("config"))
            .env("MY_HARNESS_PARTS", self.at("state"))
            .env_remove("CODEX_HOME")
            .output()
            .unwrap()
    }
    fn ok(&self, args: &[&str]) -> String {
        let out = self.run(args);
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(out.status.success(), "{args:?} failed: {text}");
        text
    }
}

fn tree(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = vec![];
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            match p.is_dir() {
                true => stack.push(p),
                false => out.push((p.clone(), fs::read(&p).unwrap())),
            }
        }
    }
    out.sort();
    out
}

#[test]
fn store_verbs_status_and_harvest_through_the_binary() {
    let e = Env::new();
    let personal = e.at("stores/personal");
    e.ok(&[
        "memory",
        "add-store",
        "personal",
        personal.to_str().unwrap(),
        "--default",
    ]);
    e.ok(&[
        "memory",
        "add-store",
        "work",
        e.at("stores/work").to_str().unwrap(),
    ]);
    let status = e.ok(&["memory", "status"]);
    assert!(
        status.contains("store personal") && status.contains("(default, maid-harness)"),
        "{status}"
    );

    let missing = e.run(&["memory", "add-store", "x", e.at("nope").to_str().unwrap()]);
    assert!(!missing.status.success());

    let mem = e.at("home/.claude/projects/-proj/memory");
    fs::create_dir_all(&mem).unwrap();
    fs::write(
        mem.join("a.md"),
        "---\nname: a\ndescription: d\nmetadata:\n  type: user\n---\nA\n",
    )
    .unwrap();
    fs::write(mem.join("MEMORY.md"), "- [a](a.md) - d\n").unwrap();
    let originals = tree(&e.at("home"));

    // Not configured: refused, nothing written anywhere.
    let refused = e.run(&["memory", "harvest", "--to", "elsewhere"]);
    assert_eq!(refused.status.code(), Some(2));
    assert!(tree(&e.at("stores")).is_empty());

    let first = e.ok(&["memory", "harvest"]);
    assert!(first.contains("copied"), "{first}");
    assert!(personal.join("memory/a.md").is_file());
    assert!(!e.at("stores/work/memory").exists());
    let after = tree(&e.at("stores"));
    let second = e.ok(&["memory", "harvest"]);
    assert!(second.contains("nothing new to harvest"), "{second}");
    assert_eq!(tree(&e.at("stores")), after);
    assert_eq!(tree(&e.at("home")), originals);

    // A store whose checkout is gone is refused, not recreated.
    fs::remove_dir_all(e.at("stores/work")).unwrap();
    assert!(!e
        .run(&["memory", "harvest", "--to", "work"])
        .status
        .success());
    assert!(!e.at("stores/work").exists());

    e.ok(&["memory", "remove-store", "personal"]);
    assert!(e.ok(&["memory", "status"]).contains("store work"));
}
