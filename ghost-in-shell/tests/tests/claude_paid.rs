//! Paid: Claude Code with a real model, to settle what a request cannot
//! show, that the model acts on what the adapter places. Two runs each for
//! the first two tests, each stopping at its first failure; four for the
//! first comparison with and without candidates, two for the second.
//!
//! The model tests are ignored, so `just test` runs only the free tests.
//! Run them by hand, one test at a time and only as granted, in the dev
//! shell (`nix develop`, for the memory server), through the user's own
//! `claude` launcher:
//!
//! ```text
//! MAID_PAID=1 CLAUDE_BIN=$(command -v claude) \
//!   cargo test -p ghost-in-shell-tests --test claude_paid <test> -- --ignored --test-threads 1
//! ```
//!
//! The launcher, not the raw Claude Code binary: the launcher is what
//! supplies the model access, and the raw binary run alone fails to
//! authenticate with the model provider. Claude Code keeps the shell's
//! HOME and environment, so its model access is the shell's own and no
//! credential is handed over. Three locations go to
//! a temp world: `CLAUDE_CONFIG_DIR`, `GHOST_IN_SHELL_STATE`
//! and `XDG_CACHE_HOME`, where Claude Code keeps its MCP logs whatever its
//! config dir. A run refuses to start if any of them, or the world, would
//! land under HOME. On a stub of the model API, nothing else was written
//! under HOME; other inherited locations are not redirected. A login kept
//! only in the real Claude config dir is not seen there; model access from
//! the environment is.

use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

use ghost_in_shell_tests::ClaudeHome;
const RUNS: usize = 2;
const MARKER: &str = "DEPLOY-MARKER-7";

/// A temp world with Claude's config dir holding mAId's part (a memory on a
/// store, a turn skill whose first rule asks for a marker), the ghost's
/// state, and a plugin built from the repo's Claude adapter files with the
/// cargo-built core.
fn world() -> TempDir {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    fs::create_dir_all(d.join("claude-config")).unwrap();
    fs::create_dir_all(d.join("cache")).unwrap();
    fs::create_dir_all(d.join("work")).unwrap();
    // mAId's part as Claude Code holds it: a plugin with the turn skill and
    // the memory's declaration, and the memory registered on its store.
    let kit = d.join("units/maid");
    fs::create_dir_all(kit.join(".claude-plugin")).unwrap();
    fs::write(
        kit.join(".claude-plugin/plugin.json"),
        json!({"name": "kit", "version": "1.0.0", "repository": "https://example.org/me/maid"})
            .to_string(),
    )
    .unwrap();
    fs::create_dir_all(kit.join("skills/deployer")).unwrap();
    fs::write(kit.join("skills/deployer/SKILL.md"), format!("---\nname: deployer\ndescription: deploy things\nmetadata:\n  ghost-in-shell.contract: \"1\"\n  ghost-in-shell.scope: turn\n  ghost-in-shell.job: deploy a release\n  ghost-in-shell.signals.jobs: deploy\n  ghost-in-shell.rules: SKILL.md#rules\n---\n## Rules\n\nThe first line of every reply about a deploy is the literal line {MARKER}.\n")).unwrap();
    fs::create_dir_all(kit.join("servers/personal-memory")).unwrap();
    fs::copy(
        ghost_in_shell_tests::content().join("servers/personal-memory/server.json"),
        kit.join("servers/personal-memory/server.json"),
    )
    .unwrap();
    let claude = ClaudeHome::in_dir(&d.join("claude-config"));
    claude.plugin("kit@kit", &kit);
    let store = d.join("personal-store");
    fs::create_dir_all(store.join("memory")).unwrap();
    claude.mcp(
        "personal-memory",
        "/bin/true",
        &["--store", &store.display().to_string()],
    );
    ghost_in_shell_tests::staged_ghost(d);
    dir
}

/// Every place a run writes, each refused if it is or resolves under
/// `home`: a run keeps the real HOME, so nothing may land there.
fn outside(home: &Path, places: &[PathBuf]) -> Result<(), String> {
    let home = home
        .canonicalize()
        .map_err(|e| format!("HOME {}: {e}", home.display()))?;
    for p in places {
        let at = p
            .canonicalize()
            .map_err(|e| format!("{}: {e}", p.display()))?;
        if at.starts_with(&home) {
            return Err(format!("{} is under HOME {}", at.display(), home.display()));
        }
    }
    Ok(())
}

fn ask(world: &Path, prompt: &str) -> String {
    ask_with(world, prompt, &[], &[])
}

/// `ask`, with more arguments for `claude` and more environment for it and
/// its hooks.
fn ask_with(world: &Path, prompt: &str, args: &[&str], env: &[(&str, &str)]) -> String {
    assert_eq!(
        std::env::var("MAID_PAID").as_deref(),
        Ok("1"),
        "paid: set MAID_PAID=1"
    );
    let (config, state, cache) = (
        world.join("claude-config"),
        world.join("state"),
        world.join("cache"),
    );
    let home = std::env::var_os("HOME").expect("paid: HOME is the shell's own");
    if let Err(why) = outside(
        Path::new(&home),
        &[world.into(), config.clone(), state.clone(), cache.clone()],
    ) {
        panic!("paid: refused before any model call: {why}");
    }
    let claude = std::env::var("CLAUDE_BIN").unwrap_or_else(|_| "claude".into());
    let mut cmd = Command::new(claude);
    cmd.args(["-p", prompt, "--output-format", "text"])
        .arg("--plugin-dir")
        .arg(world.join("ghost").join(shell_claude::package::PLUGIN_DIR))
        .current_dir(world.join("work"))
        .env("CLAUDE_CONFIG_DIR", &config)
        .env("GHOST_IN_SHELL_STATE", &state)
        .env("XDG_CACHE_HOME", &cache)
        .args(args)
        .envs(env.iter().copied());
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "claude: {}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
#[ignore = "paid: two model runs"]
fn a_fresh_session_knows_its_persona_and_store_from_the_genome() {
    for run in 1..=RUNS {
        let w = world();
        let store = w.path().join("personal-store");
        let reply = ask(
            w.path(),
            "In one line, without using any tool: which persona are you acting through, and at what path is its memory store?",
        );
        assert!(
            reply.contains("personal") && reply.contains(&store.display().to_string()),
            "run {run}: {reply}"
        );
    }
}

#[test]
#[ignore = "paid: two model runs"]
fn a_fitting_bring_ins_first_rule_is_followed() {
    for run in 1..=RUNS {
        let w = world();
        let reply = ask(
            w.path(),
            "Without using any tool: in two lines, how would you deploy the release of a small static site?",
        );
        assert!(reply.trim_start().starts_with(MARKER), "run {run}: {reply}");
    }
}

/// A saved habit the paid comparisons offer: its index line's title and
/// hook, and its text.
struct Habit {
    title: &'static str,
    file: &'static str,
    hook: &'static str,
    text: &'static str,
}

/// Macro run 5's habit. It cannot show whether candidates help: the
/// model's own fix already avoids awk, and its title's "scripts" matches
/// the prompt's "script", so it is offered with candidates off too.
const NO_AWK: Habit = Habit {
    title: "No awk in scripts",
    file: "no-awk-in-scripts.md",
    hook: "shell habit",
    text: "Never use awk in shell scripts. Use cut, sed, or a while-read loop instead.",
};

/// A habit against the model's default fix (`cut`), so keeping it shows
/// the model read it. No word of its title or hook is in `HABIT_PROMPT`:
/// it reaches the prefix only as a candidate, through the named file's
/// language (`the_discriminating_habit_reaches_the_prefix_only_as_a_candidate`).
const ALWAYS_AWK: Habit = Habit {
    title: "Always awk, never cut",
    file: "always-awk-never-cut.md",
    hook: "shell habit",
    text: "In my shell scripts always use awk to pick fields. Never use cut.",
};

/// A temp world for Macro run 5's case: no turn bring-in, a memory server
/// (`server`) on a store holding one habit, and a work folder with a shell
/// script whose fix is one line.
fn habit_world(habit: &Habit, server: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    for sub in ["claude-config", "cache", "state", "work"] {
        fs::create_dir_all(d.join(sub)).unwrap();
    }
    fs::write(d.join("work/data.sh"), "#!/bin/sh\ncat data.txt\n").unwrap();
    fs::write(d.join("work/data.txt"), "a 1 x\nb 2 y\n").unwrap();
    let kit = d.join("units/maid");
    fs::create_dir_all(kit.join(".claude-plugin")).unwrap();
    fs::write(
        kit.join(".claude-plugin/plugin.json"),
        json!({"name": "kit", "version": "1.0.0", "repository": "https://example.org/me/maid"})
            .to_string(),
    )
    .unwrap();
    fs::create_dir_all(kit.join("servers/personal-memory")).unwrap();
    fs::copy(
        ghost_in_shell_tests::content().join("servers/personal-memory/server.json"),
        kit.join("servers/personal-memory/server.json"),
    )
    .unwrap();
    let claude = ClaudeHome::in_dir(&d.join("claude-config"));
    claude.plugin("kit@kit", &kit);
    let store = d.join("personal-store");
    fs::create_dir_all(store.join("memory")).unwrap();
    fs::write(store.join("about.md"), "The personal persona's store: my own scripts and repos. Any session on my own work fits it.\n").unwrap();
    fs::write(
        store.join("memory/MEMORY.md"),
        format!("- [{}]({}) - {}\n", habit.title, habit.file, habit.hook),
    )
    .unwrap();
    fs::write(
        store.join("memory").join(habit.file),
        format!(
            "---\nname: {}\ndescription: {}\nmetadata:\n  type: feedback\n---\n\n{}\n",
            habit.title, habit.hook, habit.text
        ),
    )
    .unwrap();
    for args in [
        &["init", "-q"][..],
        &["add", "-A"],
        &["commit", "-qm", "start"],
    ] {
        git(&store, args);
    }
    claude.mcp(
        "personal-memory",
        server,
        &["--store", &store.display().to_string()],
    );
    ghost_in_shell_tests::staged_ghost(d);
    dir
}

/// `git` in `dir` with no config but its own: no user, system or global
/// config, hooks or signing from the real HOME.
fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.org"])
        .args([
            "-c",
            "init.defaultBranch=main",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

/// The real memory server, for a model run.
fn pensieve() -> String {
    std::env::var("MAID_TEST_PENSIEVE")
        .expect("MAID_TEST_PENSIEVE is not set: run in the dev shell (nix develop)")
}

const HABIT_PROMPT: &str = "Fix data.sh so it prints only the second field of each line of data.txt. Reply with the whole corrected script and nothing else.";

/// Whether a reply is a fixed script that keeps the habit.
fn keeps_the_habit(reply: &str) -> bool {
    reply.contains("data.txt") && !reply.to_lowercase().contains("awk")
}

/// Whether a reply is a fixed script that keeps `ALWAYS_AWK`.
fn keeps_always_awk(reply: &str) -> bool {
    reply.contains("data.txt") && reply.contains("awk") && !reply.contains("cut ")
}

/// One model session on `habit`'s world with candidates `on` or off.
/// `Read` is not limited to the world: the session keeps the real HOME,
/// and the fixture is trusted.
fn ask_habit(habit: &Habit, on: bool) -> String {
    let w = habit_world(habit, &pensieve());
    let store = w.path().join("personal-store").display().to_string();
    ask_with(
        w.path(),
        HABIT_PROMPT,
        &[
            "--add-dir",
            &store,
            "--allowedTools",
            "Read,mcp__personal-memory__memory_search,mcp__personal-memory__memory_view",
        ],
        &[(ghost::config::CANDIDATES, switch(on))],
    )
}

fn switch(on: bool) -> &'static str {
    match on {
        true => "on",
        false => "off",
    }
}

/// The first comparison, on Macro run 5's habit, as run four times. It
/// cannot tell the arms apart (see `NO_AWK`); kept as it was run.
#[test]
#[ignore = "paid: four model runs"]
fn with_candidates_macro_run_5s_habit_is_applied() {
    let mut table = vec![
        "| run | candidates | kept the habit |".to_string(),
        "|---|---|---|".to_string(),
    ];
    for (run, with) in [(1, true), (2, false), (3, true), (4, false)] {
        let reply = ask_habit(&NO_AWK, with);
        let kept = keeps_the_habit(&reply);
        table.push(format!(
            "| {run} | {} | {} |",
            switch(with),
            if kept { "yes" } else { "no" }
        ));
        eprintln!(
            "run {run}, candidates {}, kept the habit: {kept}\n{reply}\n",
            switch(with)
        );
        assert!(
            !with || kept,
            "run {run} with candidates missed the habit:\n{}\n{reply}",
            table.join("\n")
        );
    }
    eprintln!("{}", table.join("\n"));
}

/// The pair that can tell: a habit against the model's default. Asserts
/// the run with candidates; reports the run without.
#[test]
#[ignore = "paid: two model runs"]
fn with_candidates_a_habit_against_the_default_is_applied() {
    let on = ask_habit(&ALWAYS_AWK, true);
    let kept_on = keeps_always_awk(&on);
    eprintln!("candidates on, kept the habit: {kept_on}\n{on}\n");
    assert!(kept_on, "with candidates the habit was missed:\n{on}");
    let off = ask_habit(&ALWAYS_AWK, false);
    eprintln!(
        "candidates off, kept the habit: {}\n{off}\n",
        keeps_always_awk(&off)
    );
}

/// The prefix the ghost's prompt hook gives `HABIT_PROMPT` in `habit`'s
/// world, as Claude Code would run it there, without a model.
fn habit_prefix(habit: &Habit, on: bool) -> String {
    let w = habit_world(habit, "/bin/true");
    let d = w.path();
    let mut child = Command::new(ghost_in_shell_tests::core())
        .args([
            "expression",
            "--carrier",
            "append",
            "--input",
            "-",
            "--claude-event",
            "UserPromptSubmit",
        ])
        .env_clear()
        .env("HOME", d)
        .env("CLAUDE_CONFIG_DIR", d.join("claude-config"))
        .env("GHOST_IN_SHELL_STATE", d.join("state"))
        .env(ghost::config::CANDIDATES, switch(on))
        .current_dir(d.join("work"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let payload = json!({"session_id": "s1", "transcript_path": d.join("t.jsonl"),
        "cwd": d.join("work"), "hook_event_name": "UserPromptSubmit", "prompt": HABIT_PROMPT});
    std::io::Write::write_all(
        &mut child.stdin.take().unwrap(),
        payload.to_string().as_bytes(),
    )
    .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap()
}

/// `NO_AWK` is left out: the prompt's own words reach it (see there).
#[test]
fn the_discriminating_habit_reaches_the_prefix_only_as_a_candidate() {
    let on = habit_prefix(&ALWAYS_AWK, true);
    assert!(on.contains(ALWAYS_AWK.title), "on: {on}");
    let off = habit_prefix(&ALWAYS_AWK, false);
    assert!(!off.contains(ALWAYS_AWK.title), "off: {off}");
}

#[test]
fn a_reply_keeps_always_awk_only_with_the_script_awk_and_no_cut() {
    assert!(keeps_always_awk("#!/bin/sh\nawk '{print $2}' data.txt\n"));
    assert!(!keeps_always_awk("#!/bin/sh\ncut -d \" \" -f 2 data.txt\n"));
    assert!(!keeps_always_awk(
        "#!/bin/sh\nawk '{print $2}' data.txt | cut -c1-9\n"
    ));
    assert!(!keeps_always_awk("I would use awk."));
}

#[test]
fn a_reply_keeps_the_habit_only_with_the_script_and_no_awk() {
    assert!(keeps_the_habit("#!/bin/sh\ncut -d' ' -f2 data.txt\n"));
    assert!(!keeps_the_habit("#!/bin/sh\nawk '{print $2}' data.txt\n"));
    assert!(!keeps_the_habit("I would use cut."));
}

#[test]
fn a_place_under_home_is_refused_and_one_outside_passes() {
    let (home, away) = (TempDir::new().unwrap(), TempDir::new().unwrap());
    let inside = home.path().join("claude-config");
    fs::create_dir(&inside).unwrap();
    // A link from outside into HOME resolves under it.
    let link = away.path().join("state");
    std::os::unix::fs::symlink(&inside, &link).unwrap();
    assert!(outside(home.path(), &[away.path().into()]).is_ok());
    assert!(outside(home.path(), &[away.path().into(), inside]).is_err());
    assert!(outside(home.path(), &[link]).is_err());
    assert!(outside(home.path(), &[home.path().into()]).is_err());
}
