//! Memory from outside: the `memory` verbs through the binary, under a
//! temp HOME, store file and state dir.

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
            .env("MAID_MEMORY_STORE", self.at("config/memory-store"))
            .env("MAID_PROFILE", self.at("profile"))
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
fn set_store_status_and_harvest_through_the_binary() {
    let e = Env::new();
    let personal = e.at("stores/personal");

    // No store set: said so.
    let status = e.ok(&["memory", "status"]);
    assert!(status.contains("no personal store set"), "{status}");

    let missing = e.run(&["memory", "set-store", e.at("nope").to_str().unwrap()]);
    assert!(!missing.status.success());
    assert!(!e.at("config/memory-store").exists());

    let set = e.ok(&["memory", "set-store", personal.to_str().unwrap()]);
    assert!(set.contains("run `just install`"), "{set}");
    let status = e.ok(&["memory", "status"]);
    assert!(
        status.contains(&format!("memory   store {}\n", personal.display())),
        "{status}"
    );
    assert!(
        status.contains("personal-memory not registered"),
        "{status}"
    );

    for (project, name) in [("-work-api", "a"), ("-home-garden", "b")] {
        let mem = e.at(&format!("home/.claude/projects/{project}/memory"));
        fs::create_dir_all(&mem).unwrap();
        fs::write(
            mem.join(format!("{name}.md")),
            format!("---\nname: {name}\ndescription: d\nmetadata:\n  type: user\n---\nA\n"),
        )
        .unwrap();
    }
    let originals = tree(&e.at("home"));

    // A dir that is not there: refused, nothing written anywhere.
    let refused = e.run(&[
        "memory",
        "harvest",
        "--to",
        e.at("elsewhere").to_str().unwrap(),
    ]);
    assert_eq!(refused.status.code(), Some(2));
    assert!(!e.at("elsewhere").exists());

    let work = e.at("stores/work");
    let first = e.ok(&[
        "memory",
        "harvest",
        "--to",
        work.to_str().unwrap(),
        "--project",
        "/work",
    ]);
    assert!(first.contains("project  -work-api"), "{first}");
    assert!(work.join("memory/a.md").is_file());
    assert!(!work.join("memory/b.md").exists());
    let after = tree(&e.at("stores"));
    let second = e.ok(&[
        "memory",
        "harvest",
        "--to",
        work.to_str().unwrap(),
        "--project",
        "/work",
    ]);
    assert!(second.contains("nothing new to harvest"), "{second}");
    assert_eq!(tree(&e.at("stores")), after);
    assert_eq!(tree(&e.at("home")), originals);
}
