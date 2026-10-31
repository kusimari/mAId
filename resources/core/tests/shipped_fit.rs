//! Turn fit and expression against the declarations this repo ships: the
//! six skills, the browser server and the document-skills overlay, read
//! from `resources/content` as a part list records them.

use maid_core::config::Config;
use maid_core::entry::{expression, Call, Carrier, Ctx, Slice};
use maid_core::environment::Host;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn content() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../content")
}

fn setup() -> (TempDir, Ctx) {
    let dir = TempDir::new().unwrap();
    let c = content().canonicalize().unwrap();
    let mut bring_ins: Vec<Value> = ["kdevkit", "kyodakit", "kreviewkit", "writing-style", "notes", "browser"]
        .iter()
        .map(|s| json!({"name": s, "unit": {"standard": "skill", "path": c.join(format!("skills/{s}/SKILL.md"))}}))
        .collect();
    bring_ins.push(json!({"name": "chrome-devtools",
        "unit": {"standard": "mcp", "path": c.join("servers/browser/server.json")}}));
    bring_ins.push(json!({"name": "document-skills",
        "unit": {"standard": "plugin", "path": c.join("overlays/document-skills/plugin.json")},
        "tenure": {"kind": "borrowed", "from": "the vendor"}}));
    let parts = dir.path().join("parts");
    fs::create_dir_all(&parts).unwrap();
    let list = json!({"format": 1, "part": "maid-harness", "persona": "personal",
        "harnesses": {}, "bring_ins": bring_ins});
    fs::write(parts.join("maid-harness.json"), list.to_string()).unwrap();
    let ctx = Ctx {
        config: Config {
            parts,
            state: dir.path().join("state"),
            claude: None,
        },
        host: Host::default(),
    };
    (dir, ctx)
}

fn fits(ctx: &Ctx, prompt: &str, cwd: &Path) -> Vec<(String, String)> {
    let call = Call {
        session: None,
        carrier: Carrier::Transient,
        reset: false,
        input: json!({"prompt": prompt, "cwd": cwd}),
        slice: Slice::Whole,
    };
    expression(ctx, &call)
        .unwrap()
        .into_iter()
        .filter(|i| i.id.starts_with("fit."))
        .map(|i| (i.id, i.text))
        .collect()
}

#[test]
fn the_shipped_declarations_fit_the_work_they_name() {
    let (dir, ctx) = setup();
    let repo = dir.path().join("repo");
    fs::create_dir_all(repo.join(".git")).unwrap();
    fs::write(repo.join(".git/HEAD"), "ref: refs/heads/feat/hooks/work\n").unwrap();
    fs::create_dir_all(repo.join("specs/feature")).unwrap();
    fs::write(repo.join("specs/project.md"), "# Project\n").unwrap();
    let plain = dir.path().join("plain");
    fs::create_dir_all(&plain).unwrap();

    let spec = fits(&ctx, "plan a feature for the parser", &repo);
    assert_eq!(spec[0].0, "fit.maid-harness.kdevkit", "{spec:?}");
    assert!(
        spec[0].1.contains("### Phase-gating cues"),
        "kdevkit's rules section:\n{}",
        spec[0].1
    );

    let write = fits(&ctx, "rewrite this paragraph", &plain);
    assert_eq!(write.len(), 1, "{write:?}");
    assert_eq!(write[0].0, "fit.maid-harness.writing-style");

    let form = fits(&ctx, "fill a form on the booking page", &plain);
    assert_eq!(form[0].0, "fit.maid-harness.browser");

    let docs = fits(&ctx, "sum the totals in report.xlsx", &plain);
    assert_eq!(docs[0].0, "fit.maid-harness.document-skills");

    assert!(fits(&ctx, "hello there", &plain).is_empty(), "nothing fits");
}

#[test]
fn kdevkit_points_at_the_feature_spec_for_the_branch() {
    let (dir, ctx) = setup();
    let repo = dir.path().join("repo");
    fs::create_dir_all(repo.join(".git")).unwrap();
    fs::write(repo.join(".git/HEAD"), "ref: refs/heads/feat/hooks/work\n").unwrap();
    fs::create_dir_all(repo.join("specs/feature")).unwrap();
    for f in ["hooks", "other"] {
        fs::write(
            repo.join(format!("specs/feature/{f}.md")),
            format!("# Feature: {f}\n\n## Handoff\n\n- **Stage:** dev\n- **Ready for:** review\n"),
        )
        .unwrap();
    }
    let call = Call {
        session: None,
        carrier: Carrier::Transient,
        reset: false,
        input: json!({"prompt": "carry on", "cwd": repo}),
        slice: Slice::Whole,
    };
    let items = expression(&ctx, &call).unwrap();
    let text = &items[0].text;
    assert!(text.contains("specs/feature/hooks.md"), "{text}");
    assert!(
        text.contains("goal: Feature: hooks\n  state: dev\n  next: review"),
        "{text}"
    );
}

/// Claude Code places a hook's context whole only up to 10,000 characters,
/// and the genome is not cut, so its size for this repo's own install, with
/// a second part beside it, is held well under that.
const GENOME_BYTES: usize = 8_000;

#[test]
fn the_genome_of_this_repos_install_with_a_second_part_stays_small() {
    let (dir, ctx) = setup();
    let c = content().canonicalize().unwrap();
    let parts = dir.path().join("parts");
    let store = dir.path().join("personal-store");
    fs::create_dir_all(&store).unwrap();
    let adapter = dir.path().join("plugin.json");
    let decl: Value = serde_json::from_str(
        &fs::read_to_string(c.join("adapters/claude/my-harness.json")).unwrap(),
    )
    .unwrap();
    fs::write(
        &adapter,
        json!({"name": "maid", "extensions": {"my-harness": decl}}).to_string(),
    )
    .unwrap();
    let mine = parts.join("maid-harness.json");
    let mut list: Value = serde_json::from_str(&fs::read_to_string(&mine).unwrap()).unwrap();
    let more = list["bring_ins"].as_array_mut().unwrap();
    more.push(json!({"name": "personal-memory",
        "unit": {"standard": "mcp", "path": c.join("servers/personal-memory/server.json")},
        "values": {"store": store, "index": "memory/MEMORY.md"}}));
    more.push(json!({"name": "claude-adapter", "unit": {"standard": "plugin", "path": adapter}}));
    fs::write(&mine, list.to_string()).unwrap();
    let other = json!({"format": 1, "part": "work-part", "persona": "work", "harnesses": {},
        "bring_ins": [
            {"name": "work-memory", "unit": {"standard": "mcp", "path": c.join("servers/personal-memory/server.json")},
                "values": {"store": dir.path().join("work-store"), "index": "memory/MEMORY.md"}},
            {"name": "work-notes", "unit": {"standard": "skill", "path": c.join("skills/notes/SKILL.md")}},
            {"name": "work-review", "unit": {"standard": "skill", "path": c.join("skills/kreviewkit/SKILL.md")}}]});
    fs::write(parts.join("work-part.json"), other.to_string()).unwrap();
    let call = Call {
        session: None,
        carrier: Carrier::Transient,
        reset: false,
        input: json!({"cwd": dir.path()}),
        slice: Slice::Whole,
    };
    let text: String = maid_core::entry::genome(&ctx, &call)
        .unwrap()
        .into_iter()
        .map(|i| i.text)
        .collect();
    eprintln!(
        "genome: {} bytes, {} lines",
        text.len(),
        text.lines().count()
    );
    assert!(
        text.contains("personal-memory") && text.contains("work-review"),
        "{text}"
    );
    assert!(text.len() <= GENOME_BYTES, "{} bytes:\n{text}", text.len());
}
