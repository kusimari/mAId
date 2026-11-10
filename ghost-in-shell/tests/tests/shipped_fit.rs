//! Turn fit and expression against the declarations this repo ships: the
//! six skills, the browser server and the document-skills overlay, in
//! mAId's plugin as Claude Code holds it, read back through this shell's
//! adapter. Spans crates: `infusion`'s entry points over this shell.

use ghost::config::Config;
use ghost::environment::Host;
use ghost_in_shell_tests::{maid_plugin, ClaudeHome};
use infusion::call::{Call, Carrier, Ctx, Where};
use infusion::expression;
use serde_json::Value;
use shell_claude::config_dir::ConfigDir;
use shell_claude::installed::Claude;
use shell_claude::transcript::Transcripts;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn content() -> PathBuf {
    ghost_in_shell_tests::content()
}

/// This repo's install as Claude Code holds it: mAId's plugin, the vendor's
/// document skills and the browser server, read back by the adapter.
fn setup() -> (TempDir, Ctx) {
    let dir = TempDir::new().unwrap();
    let claude = ClaudeHome::at(&dir.path().join("home"));
    claude.plugin("maid@maid", &maid_plugin(&dir.path().join("plugin")));
    let vendor = dir.path().join("vendor");
    fs::create_dir_all(vendor.join(".claude-plugin")).unwrap();
    fs::write(
        vendor.join(".claude-plugin/plugin.json"),
        r#"{"name":"document-skills"}"#,
    )
    .unwrap();
    claude.plugin("document-skills@anthropic-agent-skills", &vendor);
    claude.mcp("chrome-devtools", "/bin/true", &[]);
    let conf = dir.path().join("config/maid");
    let ctx = Ctx {
        config: Config {
            state: dir.path().join("state"),
        },
        host: Host::default(),
        sources: Box::new(Transcripts { projects: None }),
        shell: Box::new(Claude {
            config: Some(ConfigDir::new(&claude.home, None)),
            cache: None,
            vars: Some([("MAID_CONFIG".to_string(), conf.display().to_string())].into()),
        }),
    };
    (dir, ctx)
}

fn fits(ctx: &Ctx, prompt: &str, cwd: &Path) -> Vec<(String, String)> {
    let call = Call {
        carrier: Carrier::Transient,
        prompt: Some(prompt.into()),
        at: Where {
            cwd: Some(cwd.to_path_buf()),
            ..Where::default()
        },
        ..Call::default()
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
    assert_eq!(spec[0].0, "fit.mAId.kdevkit", "{spec:?}");
    assert!(
        spec[0].1.contains("### Phase-gating cues"),
        "kdevkit's rules section:\n{}",
        spec[0].1
    );

    let write = fits(&ctx, "rewrite this paragraph", &plain);
    assert_eq!(write.len(), 1, "{write:?}");
    assert_eq!(write[0].0, "fit.mAId.writing-style");

    let form = fits(&ctx, "fill a form on the booking page", &plain);
    assert_eq!(form[0].0, "fit.mAId.browser");

    let docs = fits(&ctx, "sum the totals in report.xlsx", &plain);
    assert_eq!(docs[0].0, "fit.mAId.document-skills");

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
        carrier: Carrier::Transient,
        prompt: Some("carry on".into()),
        at: Where {
            cwd: Some(repo.clone()),
            ..Where::default()
        },
        ..Call::default()
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
/// a second part beside it and writing-style's files bound, is held well
/// under that.
const GENOME_BYTES: usize = 8_000;

#[test]
fn the_genome_of_this_repos_install_with_a_second_part_stays_small() {
    let (dir, ctx) = setup();
    let claude = ClaudeHome::at(&dir.path().join("home"));
    let store = dir.path().join("personal-store");
    fs::create_dir_all(&store).unwrap();
    let store = store.display().to_string();
    claude.mcp("personal-memory", "/bin/pensieve", &["--store", &store]);
    // A second part, its own plugin with its memory and two skills.
    let work = dir.path().join("work-plugin");
    fs::create_dir_all(work.join(".claude-plugin")).unwrap();
    fs::write(
        work.join(".claude-plugin/plugin.json"),
        r#"{"name":"work","repository":"https://example.org/me/work"}"#,
    )
    .unwrap();
    for (to, from) in [("work-notes", "notes"), ("work-review", "kreviewkit")] {
        fs::create_dir_all(work.join("skills").join(to)).unwrap();
        fs::copy(
            content().join(format!("skills/{from}/SKILL.md")),
            work.join(format!("skills/{to}/SKILL.md")),
        )
        .unwrap();
    }
    let mut server: Value = serde_json::from_str(
        &fs::read_to_string(content().join("servers/personal-memory/server.json")).unwrap(),
    )
    .unwrap();
    server["_meta"]["my-harness"]["persona"] = "work".into();
    fs::create_dir_all(work.join("servers/work-memory")).unwrap();
    fs::write(
        work.join("servers/work-memory/server.json"),
        server.to_string(),
    )
    .unwrap();
    claude.plugin("work@work", &work);
    let work_store = dir.path().join("work-store").display().to_string();
    claude.mcp("work-memory", "/bin/pensieve", &["--store", &work_store]);
    let conf = dir.path().join("config/maid/writing-style");
    let call = Call {
        carrier: Carrier::Transient,
        at: Where {
            cwd: Some(dir.path().to_path_buf()),
            ..Where::default()
        },
        ..Call::default()
    };
    let text: String = infusion::genome(&ctx, &call)
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
    // writing-style's older files are named where they are bound, so the
    // skill never reads a default path.
    let style = text
        .lines()
        .find(|l| l.starts_with("  writing-style: "))
        .unwrap_or_default();
    assert!(
        style.ends_with(&format!(
            "; locations: learned {}, promoted {}",
            conf.join("learned.md").display(),
            conf.join("promoted.md").display()
        )),
        "{style}"
    );
    assert!(text.len() <= GENOME_BYTES, "{} bytes:\n{text}", text.len());
}
