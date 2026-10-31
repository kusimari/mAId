use super::*;
use crate::claude::Claude;
use crate::source::SessionSource;
use std::path::Path;
use tempfile::TempDir;

struct World {
    _dir: TempDir,
    root: PathBuf,
    config: Config,
    map: Map,
    bundle: Bundle,
    log: PathBuf,
}

impl World {
    fn new() -> World {
        World::with_serve(None)
    }

    /// A personal part (its memory, writing-style, a borrowed overlay, the
    /// adapter) and a work part with its own memory, beside the fixture
    /// session.
    fn with_serve(serve: Option<String>) -> World {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        let config = Config {
            parts: root.join("parts"),
            state: root.join("state"),
            claude: Some(crate::fixtures::claude_projects(&root)),
        };
        let (stub, log) = crate::memory::tests::stub(&root);
        let serve = serve.unwrap_or(stub);
        let units = root.join("units");
        fs::create_dir_all(&units).unwrap();
        let mem = |name: &str| {
            let p = units.join(format!("{name}.json"));
            let decl = json!({"scope": "session", "job": format!("{name} memory"), "package": "pensieve",
                "config": {"store": {"about": "s"}, "index": {"about": "i"}, "serve": {"about": "c"}},
                "state": {"permanent": "store", "promote": "off"}});
            fs::write(
                &p,
                json!({"name": name, "_meta": {"my-harness": decl}}).to_string(),
            )
            .unwrap();
            p
        };
        let style = units.join("writing-style.md");
        fs::write(
            &style,
            "---\nname: writing-style\ndescription: d\nmetadata:\n  my-harness.scope: turn\n  my-harness.job: write in my voice\n  my-harness.config.learned: \"[optional] pending\"\n  my-harness.config.promoted: \"[optional] promoted\"\n  my-harness.state.local: learned\n  my-harness.state.permanent: promoted\n  my-harness.state.promote: propose\n---\n",
        )
        .unwrap();
        let plugin = |name: &str| {
            let p = units.join(format!("{name}.plugin.json"));
            fs::write(
                &p,
                json!({"name": name, "extensions": {"my-harness": {"scope": "turn", "job": format!("{name} job")}}})
                    .to_string(),
            )
            .unwrap();
            p
        };
        let store = root.join("store");
        let work_store = root.join("work-store");
        fs::create_dir_all(store.join("memory")).unwrap();
        fs::create_dir_all(work_store.join("memory")).unwrap();
        let conf = root.join("conf/writing-style");
        fs::create_dir_all(&conf).unwrap();
        let project =
            json!({"kind": "project", "at": "https://example.org/me/maid", "rev": "abc123"});
        let personal = json!({"format": 1, "part": "maid-harness", "persona": "personal", "harnesses": {},
        "bring_ins": [
            {"name": "personal-memory", "unit": {"standard": "mcp", "path": mem("personal-memory")},
             "source": {"kind": "project", "at": "https://example.org/me/pensieve", "rev": "p1"},
             "values": {"store": store, "index": "memory/MEMORY.md", "serve": serve}},
            {"name": "writing-style", "unit": {"standard": "skill", "path": style}, "source": project,
             "values": {"learned": conf.join("learned.md"), "promoted": conf.join("promoted.md")}},
            {"name": "document-skills", "unit": {"standard": "plugin", "path": plugin("document-skills")},
             "source": {"kind": "vendor", "at": "https://example.org/vendor/skills", "rev": "v1"},
             "tenure": {"kind": "borrowed", "from": "a vendor", "since": "2026-10-01"}},
            {"name": "claude-adapter", "unit": {"standard": "plugin", "path": plugin("claude-adapter")}, "source": project}
        ]});
        let work = json!({"format": 1, "part": "work-part", "persona": "work", "harnesses": {},
            "bring_ins": [{"name": "work-memory", "unit": {"standard": "mcp", "path": mem("work-memory")},
                "values": {"store": work_store, "index": "memory/MEMORY.md", "serve": "[\"/nowhere/server\"]"}}]});
        fs::create_dir_all(&config.parts).unwrap();
        fs::write(config.parts.join("maid-harness.json"), personal.to_string()).unwrap();
        fs::write(config.parts.join("work-part.json"), work.to_string()).unwrap();
        let map = crate::map::fuse(&config.parts).unwrap();
        let claude = Claude {
            root: config.claude.clone().unwrap(),
        };
        let at = claude
            .locate(Some(crate::fixtures::SESSION), &Value::Null)
            .unwrap();
        let bundle = claude.pack(&at).unwrap();
        World {
            _dir: dir,
            root,
            config,
            map,
            bundle,
            log,
        }
    }

    fn check(&self, findings: Value) -> (Vec<Pending>, Vec<Dropped>) {
        let r: Reflection = serde_json::from_value(json!({ "findings": findings })).unwrap();
        check(&self.config, &self.map, &self.bundle, r).unwrap()
    }

    fn decide(&self, answer: Value) -> Vec<Outcome> {
        decide(&self.config, &self.map, &self.bundle.session, &answer).unwrap()
    }

    fn pending_count(&self) -> usize {
        fs::read_dir(self.config.pending())
            .map(|rd| rd.count())
            .unwrap_or(0)
    }

    fn learned(&self) -> PathBuf {
        self.root.join("conf/writing-style/learned.md")
    }

    /// Every file under the temp root but the state dir and the server's
    /// log, with its bytes.
    fn snapshot(&self) -> Vec<(PathBuf, Vec<u8>)> {
        let mut out = vec![];
        walk(&self.root, &mut out);
        out.retain(|(p, _)| !p.starts_with(&self.config.state) && *p != self.log);
        out.sort();
        out
    }
}

fn walk(dir: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        match p.is_dir() {
            true => walk(&p, out),
            false => out.push((p.clone(), fs::read(&p).unwrap())),
        }
    }
}

const CORRECTION: &str = "no, use tr, not sed: we never use sed in these scripts";
const TAUGHT: &str = "new rule: no em dashes in commit messages";
const WEB: &str = "always write commit subjects in the imperative mood";

fn habit_memory() -> Value {
    json!({"thing": "personal-memory", "kind": "habit", "title": "Use tr, not sed",
        "type": "feedback", "change": "shell scripts use tr, not sed",
        "entry": "In shell scripts, use tr for case changes; never sed.",
        "evidence": [{"quote": CORRECTION, "at": "said"}]})
}

fn habit_style() -> Value {
    json!({"thing": "maid-harness/writing-style", "kind": "habit", "tier": "local",
        "change": "no em dashes in commit messages", "entry": "no em dashes in commit messages",
        "evidence": [{"quote": TAUGHT}]})
}

fn nature_style() -> Value {
    json!({"thing": "writing-style", "kind": "nature",
        "change": "say in the skill that commit messages follow the style rules too",
        "evidence": [{"quote": TAUGHT}]})
}

#[test]
fn evidence_must_be_quoted_word_for_word_from_the_session() {
    let w = World::new();
    let (kept, dropped) = w.check(json!([
        {"thing": "writing-style", "kind": "nature", "change": "a", "evidence": []},
        {"thing": "writing-style", "kind": "nature", "change": "b", "evidence": [{"quote": "use tr"}]},
        {"thing": "writing-style", "kind": "nature", "change": "c", "evidence": [{"quote": "the person said something else"}]},
        {"thing": "writing-style", "kind": "nature", "change": "d",
         "evidence": [{"quote": "no, use tr,\n   not sed: we never"}]}
    ]));
    let why: Vec<&str> = dropped.iter().map(|d| d.why.as_str()).collect();
    assert_eq!(why[0], "no evidence");
    assert!(why[1].contains("too short"), "{}", why[1]);
    assert!(why[2].contains("is not in the session"), "{}", why[2]);
    assert_eq!(kept.len(), 1, "whitespace does not matter");
}

#[test]
fn a_finding_must_name_a_thing_in_the_harness_with_a_tier_for_a_habit() {
    let w = World::new();
    let ev = json!([{"quote": CORRECTION}]);
    let (kept, dropped) = w.check(json!([
        {"thing": "nonesuch", "kind": "nature", "change": "a", "evidence": ev},
        {"thing": "claude-adapter", "kind": "habit", "change": "b", "entry": "x", "evidence": ev},
        {"thing": "harness", "kind": "habit", "change": "c", "entry": "x", "evidence": ev},
        {"kind": "habit", "change": "d", "entry": "x", "evidence": ev}
    ]));
    assert_eq!(dropped.len(), 3);
    assert!(dropped[0].why.contains("names nothing"));
    assert!(
        dropped[1].why.contains("has no local tier"),
        "{}",
        dropped[1].why
    );
    assert_eq!(kept.len(), 1);
    assert_eq!(
        kept[0].action,
        Action::Unplaced,
        "no thing, no persona: asked"
    );
    assert_eq!(kept[0].needs, [Need::Persona]);
}

#[test]
fn nature_is_only_proposed_and_harness_changes_go_to_its_source() {
    let w = World::new();
    let before = w.snapshot();
    let (kept, _) = w.check(json!([nature_style(),
        {"thing": "harness", "kind": "nature", "change": "the reflector should read tiers first",
         "evidence": [{"quote": CORRECTION}]}]));
    assert!(
        matches!(&kept[0].action, Action::Proposal { worktree: false, source: Some(s) }
        if s.at.as_deref() == Some("https://example.org/me/maid"))
    );
    assert!(matches!(&kept[1].action, Action::Proposal { .. }));
    let out = w.decide(json!("ok"));
    assert!(out.iter().all(|o| o.outcome == "accepted"), "{out:?}");
    assert_eq!(w.snapshot(), before, "no repo or skill file changes");
    let dir = w.config.proposals().join("maid-harness-writing-style");
    let file = fs::read_dir(&dir).unwrap().next().unwrap().unwrap().path();
    let text = fs::read_to_string(file).unwrap();
    assert!(text.starts_with("# Backlog: say-in-the-skill"));
    assert!(text.contains("https://example.org/me/maid at abc123"));
    assert!(text.contains(TAUGHT));
}

#[test]
fn a_worktree_needs_a_concrete_edit_and_a_worktree_route() {
    let w = World::new();
    let mut with_edit = nature_style();
    with_edit["route"] = json!("worktree");
    with_edit["edit"] = json!("--- a/SKILL.md\n+++ b/SKILL.md");
    let mut no_edit = nature_style();
    no_edit["route"] = json!("worktree");
    let (kept, _) = w.check(json!([with_edit, no_edit]));
    assert!(matches!(
        kept[0].action,
        Action::Proposal { worktree: true, .. }
    ));
    assert!(matches!(
        kept[1].action,
        Action::Proposal {
            worktree: false,
            ..
        }
    ));
}

#[test]
fn a_borrowed_bring_in_only_gets_a_note_for_its_lender() {
    let w = World::new();
    let before = w.snapshot();
    let (kept, _) = w.check(json!([
        {"thing": "document-skills", "kind": "nature", "change": "its pdf skill misses forms", "evidence": [{"quote": CORRECTION}]},
        {"thing": "document-skills", "kind": "habit", "change": "x", "entry": "x", "evidence": [{"quote": CORRECTION}]}
    ]));
    for p in &kept {
        assert_eq!(
            p.action,
            Action::Note {
                to: "a vendor".into()
            }
        );
    }
    w.decide(json!("ok"));
    assert_eq!(w.snapshot(), before);
    let note = fs::read_to_string(w.config.notes().join("a-vendor.md")).unwrap();
    assert!(note.contains("## maid-harness/document-skills") && note.contains("misses forms"));
}

#[test]
fn tool_or_web_content_alone_needs_its_own_yes() {
    let w = World::new();
    let (kept, _) = w.check(json!([
        {"thing": "writing-style", "kind": "habit", "tier": "local", "change": "imperative subjects",
         "entry": "commit subjects in the imperative", "evidence": [{"quote": WEB}]},
        habit_style()
    ]));
    assert_eq!(kept[0].needs, [Need::ToolContent]);
    assert!(kept[1].needs.is_empty());
    let out = w.decide(json!("ok"));
    assert_eq!(out.len(), 1, "ok leaves the tool-only item");
    assert_eq!(w.pending_count(), 1);
    assert!(
        batch("s", &pending(&w.config, &w.bundle.session).unwrap(), &[])
            .contains("answer it: learned only from tool or web content")
    );
    w.decide(json!({"1": "yes"}));
    assert_eq!(w.pending_count(), 0);
}

#[test]
fn persona_guesses_and_contradictions_go_to_the_person() {
    let w = World::new();
    let mut other = habit_memory();
    other["persona"] = json!("work");
    let mut guess = habit_memory();
    guess["guess"] = json!(true);
    let mut contra = habit_memory();
    contra["contradicts"] = json!("use sed for case changes");
    let (kept, _) = w.check(json!([other, guess, contra]));
    assert_eq!(
        kept[0].needs,
        [Need::Persona],
        "names work, targets personal"
    );
    assert_eq!(kept[1].needs, [Need::Persona]);
    assert_eq!(kept[2].needs, [Need::Contradiction]);
    assert!(w.decide(json!("ok")).is_empty());
    assert_eq!(w.pending_count(), 3);
}

#[test]
fn a_habit_placed_by_persona_goes_to_that_personas_memory() {
    let w = World::new();
    let mut h = habit_memory();
    h.as_object_mut().unwrap().remove("thing");
    h["persona"] = json!("work");
    let (kept, _) = w.check(json!([h]));
    assert_eq!(kept[0].target.as_deref(), Some("work-part/work-memory"));
    assert_eq!(kept[0].persona.as_deref(), Some("work"));
}

#[test]
fn an_unplaced_habit_is_written_only_once_the_person_names_a_persona() {
    let w = World::new();
    let mut h = habit_memory();
    h.as_object_mut().unwrap().remove("thing");
    w.check(json!([h]));
    assert_eq!(w.decide(json!({"1": "yes"}))[0].outcome, "waiting");
    assert!(!w.log.exists(), "nothing written");
    let out = w.decide(json!({"1": {"persona": "personal"}}));
    assert_eq!(out[0].outcome, "accepted", "{out:?}");
    assert_eq!(out[0].thing, "maid-harness/personal-memory");
    assert!(fs::read_to_string(&w.log)
        .unwrap()
        .contains("memory_create"));
}

#[test]
fn more_than_ten_findings_drop_the_rest() {
    let w = World::new();
    let many: Vec<Value> = (0..CAP + 2).map(|_| nature_style()).collect();
    let (kept, dropped) = w.check(Value::Array(many));
    assert_eq!(kept.len(), CAP);
    assert_eq!(dropped.len(), 2);
    assert!(dropped[0].why.contains("over the batch cap"));
}

#[test]
fn a_persona_habit_is_sent_to_its_memory_server_file_first_then_the_index() {
    let w = World::new();
    w.check(json!([habit_memory()]));
    let out = w.decide(json!("ok"));
    assert_eq!(out[0].outcome, "accepted", "{out:?}");
    let sent: Vec<Value> = fs::read_to_string(&w.log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .filter(|v: &Value| v["method"] == "tools/call")
        .collect();
    assert_eq!(sent[0]["params"]["name"], "memory_create");
    assert_eq!(
        sent[0]["params"]["arguments"]["path"],
        "/memories/use-tr-not-sed.md"
    );
    let body = sent[0]["params"]["arguments"]["file_text"]
        .as_str()
        .unwrap();
    assert!(body.starts_with(
        "---\nname: \"Use tr, not sed\"\ndescription: \"shell scripts use tr, not sed\"\nmetadata:\n  type: feedback\n---\n"
    ), "{body}");
    assert_eq!(
        sent[1]["params"]["name"], "memory_create",
        "no index yet: created"
    );
    assert_eq!(
        sent[1]["params"]["arguments"]["file_text"],
        "- [Use tr, not sed](use-tr-not-sed.md) - shell scripts use tr, not sed\n"
    );
}

#[test]
fn with_an_index_the_line_is_inserted_at_its_end_and_a_remove_takes_both_out() {
    let w = World::new();
    let index = w.root.join("store/memory/MEMORY.md");
    fs::write(&index, "- [A](a.md) - a\n- [B](b.md) - b\n").unwrap();
    w.check(json!([habit_memory()]));
    w.decide(json!("ok"));
    let calls = |w: &World| -> Vec<Value> {
        fs::read_to_string(&w.log)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str::<Value>(l).unwrap())
            .filter(|v| v["method"] == "tools/call")
            .map(|v| v["params"].clone())
            .collect()
    };
    let sent = calls(&w);
    assert_eq!(sent[1]["name"], "memory_insert");
    assert_eq!(sent[1]["arguments"]["insert_line"], 2);
    fs::write(
        &index,
        "- [A](a.md) - a\n- [Use tr, not sed](use-tr-not-sed.md) - x\n",
    )
    .unwrap();
    fs::remove_file(&w.log).unwrap();
    let mut gone = habit_memory();
    gone["op"] = json!("remove");
    w.check(json!([gone]));
    w.decide(json!("ok"));
    let sent = calls(&w);
    assert_eq!(sent[0]["name"], "memory_str_replace");
    assert_eq!(
        sent[0]["arguments"]["old_str"],
        "- [Use tr, not sed](use-tr-not-sed.md) - x\n"
    );
    assert_eq!(sent[1]["name"], "memory_delete");
}

#[test]
fn a_memory_already_in_the_index_is_listed_and_not_written() {
    let w = World::new();
    fs::write(
        w.root.join("store/memory/MEMORY.md"),
        "- [Use tr, not sed](use-tr-not-sed.md) - x\n",
    )
    .unwrap();
    let (kept, _) = w.check(json!([habit_memory()]));
    assert_eq!(
        kept[0].action,
        Action::Already {
            tier: "store".into()
        }
    );
    assert!(batch("s", &kept, &[]).contains("already in store, no action"));
    assert_eq!(w.decide(json!("ok"))[0].outcome, "already");
    assert!(!w.log.exists());
}

#[test]
fn a_server_that_fails_keeps_the_item_pending_with_why() {
    let w = World::with_serve(Some("[\"/nowhere/server\"]".into()));
    w.check(json!([habit_memory()]));
    let out = w.decide(json!("ok"));
    assert_eq!(out[0].outcome, "failed");
    let left = pending(&w.config, &w.bundle.session).unwrap();
    assert_eq!(left.len(), 1);
    assert!(left[0].error.as_deref().unwrap().contains("cannot start"));
}

#[test]
fn a_tier_entry_goes_under_its_heading_newest_first_and_a_remove_takes_it_out() {
    let w = World::new();
    w.check(json!([habit_style()]));
    w.decide(json!("ok"));
    let text = fs::read_to_string(w.learned()).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "## Learned rules", "a new file gets its heading");
    assert!(
        lines[2].ends_with(" · no em dashes in commit messages"),
        "{text}"
    );
    assert!(lines[3].starts_with("  · source: reflection on session"));
    let mut other = habit_style();
    other["entry"] = json!("prefer to over in order to");
    other["evidence"] = json!([{"quote": CORRECTION}]);
    w.check(json!([other]));
    w.decide(json!("ok"));
    let text = fs::read_to_string(w.learned()).unwrap();
    assert!(
        text.lines()
            .nth(2)
            .unwrap()
            .ends_with("prefer to over in order to"),
        "{text}"
    );
    let mut gone = habit_style();
    gone["op"] = json!("remove");
    w.check(json!([gone]));
    w.decide(json!("ok"));
    let text = fs::read_to_string(w.learned()).unwrap();
    assert!(
        !text.contains("em dashes")
            && !text.contains(
                "source: reflection on session 0b7c6a8e-1111-4222-8333-944455556666\n  ·"
            ),
        "{text}"
    );
    assert_eq!(text.lines().filter(|l| l.starts_with("- ")).count(), 1);
}

#[test]
fn an_entry_the_person_already_taught_is_listed_already_in_its_tier() {
    let w = World::new();
    fs::write(
        w.learned(),
        "## Learned rules\n\n- 2026-10-31 · No em dashes in commit messages\n  · source: explicit teach\n",
    )
    .unwrap();
    let before = w.snapshot();
    let (kept, _) = w.check(json!([habit_style()]));
    assert_eq!(
        kept[0].action,
        Action::Already {
            tier: "learned".into()
        }
    );
    assert!(batch("s", &kept, &[]).contains("already in learned, no action"));
    assert_eq!(w.decide(json!("ok"))[0].outcome, "already");
    assert_eq!(w.snapshot(), before);
}

#[test]
fn none_declines_all_and_deciding_everything_closes_the_mark() {
    let w = World::new();
    let at = crate::source::Located {
        source: "claude-code".into(),
        session: w.bundle.session.clone(),
        transcript: "/t.jsonl".into(),
        subs: vec![],
    };
    crate::sleep::mark(&w.config, &at, None, "close").unwrap();
    let before = w.snapshot();
    w.check(json!([habit_memory(), habit_style(), nature_style()]));
    assert_eq!(w.pending_count(), 3);
    assert_eq!(crate::sleep::waiting(&w.config).unwrap().len(), 1);
    let out = w.decide(json!("none"));
    assert!(out.iter().all(|o| o.outcome == "declined"));
    assert_eq!(w.pending_count(), 0);
    assert_eq!(w.snapshot(), before);
    assert!(crate::sleep::waiting(&w.config).unwrap().is_empty());
}

#[test]
fn a_new_reflection_replaces_the_undecided_items_of_its_session() {
    let w = World::new();
    w.check(json!([habit_memory(), habit_style()]));
    w.check(json!([nature_style()]));
    assert_eq!(w.pending_count(), 1);
}

#[test]
fn the_batch_says_what_changes_where_and_for_whom_with_its_evidence() {
    let w = World::new();
    let (kept, dropped) = w.check(json!([habit_memory(), habit_style(), nature_style(),
        {"thing": "x", "kind": "nature", "change": "dropped one", "evidence": [{"quote": CORRECTION}]}]));
    let text = batch(&w.bundle.session, &kept, &dropped);
    for want in [
        "3 to decide",
        "  1. habit, maid-harness/personal-memory memory [personal]: add feedback \"Use tr, not sed\" (shell scripts use tr, not sed): \"In shell scripts, use tr for case changes; never sed.\"; evidence \"no, use tr",
        "  2. habit, maid-harness/writing-style learned [personal]: add \"no em dashes in commit messages\"",
        "  3. backlog item, maid-harness/writing-style (on https://example.org/me/maid): say in the skill",
        "  dropped 1:\n    dropped one: x names nothing in the harness",
    ] {
        assert!(text.contains(want), "{want:?} in:\n{text}");
    }
}

#[test]
fn the_brief_names_every_target_its_tiers_and_what_was_decided() {
    let w = World::new();
    let text = brief(
        "s",
        Path::new("/e.json"),
        "/t.jsonl",
        &w.map,
        &["a: accepted: b".into()],
    );
    for want in [
        "maid-harness/personal-memory: personal-memory memory; personal, own, route worktree; permanent memory store at ",
        "/store/memory/MEMORY.md (promote off)",
        "maid-harness/writing-style: write in my voice; personal, own, route worktree; local tier learned at ",
        "(promote propose)",
        "maid-harness/document-skills: document-skills job; personal, borrowed from a vendor, route report",
        "work-part/work-memory: work-memory memory; work, own, route report",
        "  harness: the core",
        "Already decided for this session (do not propose again):\n  a: accepted: b",
    ] {
        assert!(text.contains(want), "{want:?} in:\n{text}");
    }
}

#[test]
fn answers_read_ok_none_and_per_item() {
    let w = World::new();
    let (kept, _) = w.check(json!([habit_memory(), habit_style()]));
    assert_eq!(answers(&kept, &json!("ok")).unwrap().len(), 2);
    assert_eq!(
        answers(&kept, &json!({"2": "no", "1": {"persona": "work"}})).unwrap(),
        [(1, Answer::Persona("work".into())), (2, Answer::No)]
    );
    assert!(answers(&kept, &json!({"1": "maybe"})).is_err());
    assert!(answers(&kept, &json!(3)).is_err());
}

#[test]
fn a_habit_entry_longer_than_one_read_is_dropped() {
    let w = World::new();
    let mut long = habit_style();
    long["entry"] = json!("x".repeat(MAX_ENTRY + 1));
    let mut fits = habit_style();
    fits["entry"] = json!("y".repeat(MAX_ENTRY));
    let (kept, dropped) = w.check(json!([long, fits]));
    assert_eq!(kept.len(), 1);
    assert!(
        dropped[0].why.contains("longer than 200 characters"),
        "{}",
        dropped[0].why
    );
}

#[test]
fn dismissing_leaves_a_session_out_and_says_so() {
    let w = World::new();
    let at = |s: &str| crate::source::Located {
        source: "claude-code".into(),
        session: s.into(),
        transcript: "/t.jsonl".into(),
        subs: vec![],
    };
    crate::sleep::mark(&w.config, &at(&w.bundle.session), None, "close").unwrap();
    crate::sleep::mark(&w.config, &at("other"), None, "end").unwrap();
    w.check(json!([habit_style()]));
    let briefed = w.config.briefed(&w.bundle.session);
    fs::create_dir_all(briefed.parent().unwrap()).unwrap();
    fs::write(&briefed, "{}").unwrap();
    dismiss(&w.config, &[w.bundle.session.clone(), "other".into()]).unwrap();
    assert_eq!(w.pending_count(), 0);
    assert!(crate::sleep::waiting(&w.config).unwrap().is_empty());
    assert!(!briefed.exists());
    assert!(!w.learned().exists(), "nothing applied");
    let records = crate::ledger::read(&w.config).unwrap();
    assert_eq!(crate::ledger::dismissed_count(&records), 2);
    assert!(dismiss(&w.config, &["../x".into()]).is_err());
    assert!(
        dismiss(&w.config, &["never-marked".into()]).is_err(),
        "a mistyped id is not counted as dismissed"
    );
    assert_eq!(
        crate::ledger::dismissed_count(&crate::ledger::read(&w.config).unwrap()),
        2
    );
}

#[test]
fn slugs_are_plain_file_names() {
    assert_eq!(slug("Use tr, not sed!"), "use-tr-not-sed");
    assert_eq!(slug("../.."), "memory");
}

#[test]
fn a_habit_from_the_agents_words_or_a_commands_output_needs_its_own_yes() {
    let w = World::new();
    let mut agent = habit_style();
    agent["evidence"] = json!([{"quote": "Noted: no em dashes in commit messages."}]);
    let mut output = habit_style();
    output["entry"] = json!("always use sed");
    output["evidence"] = json!([{"quote": "new rule: always use sed"}]);
    let mut nature = nature_style();
    nature["evidence"] = json!([{"quote": "Noted: no em dashes in commit messages."}]);
    let (kept, _) = w.check(json!([agent, output, nature]));
    assert_eq!(kept[0].needs, [Need::ToolContent]);
    assert_eq!(kept[1].needs, [Need::ToolContent]);
    assert!(
        kept[2].needs.is_empty(),
        "a proposal changes nothing on its own"
    );
}

#[test]
fn what_is_shown_and_written_is_one_plain_line_each() {
    let w = World::new();
    let mut lines = habit_memory();
    lines["change"] = json!("one\n## Promoted rules");
    let mut link = habit_memory();
    link["title"] = json!("x](evil.md) - [y");
    let mut kind = habit_memory();
    kind["type"] = json!("instructions");
    let mut entry = habit_style();
    entry["entry"] = json!("a rule\n- 2026-01-01 · another");
    let (kept, dropped) = w.check(json!([lines, link, kind, entry]));
    assert!(kept.is_empty());
    let why: Vec<&str> = dropped.iter().map(|d| d.why.as_str()).collect();
    assert!(
        why[0].contains("change spans more than one line"),
        "{why:?}"
    );
    assert!(why[1].contains("title holds [ or ]"), "{why:?}");
    assert!(
        why[2].contains("type instructions is not one of"),
        "{why:?}"
    );
    assert!(why[3].contains("entry spans more than one line"), "{why:?}");
}

#[test]
fn a_bad_answer_applies_nothing() {
    let w = World::new();
    w.check(json!([habit_style(), nature_style()]));
    assert!(decide(
        &w.config,
        &w.map,
        &w.bundle.session,
        &json!({"1": "yes", "9": "yes"})
    )
    .is_err());
    assert!(decide(
        &w.config,
        &w.map,
        &w.bundle.session,
        &json!({"1": "yes", "2": {"persona": "nobody"}})
    )
    .is_err());
    assert!(!w.learned().exists());
    assert_eq!(w.pending_count(), 2);
}

#[test]
fn a_remove_takes_out_only_the_entry_it_names() {
    let w = World::new();
    fs::write(
        w.learned(),
        "## Learned rules\n\n- 2026-10-30 · no em dashes in commit messages or titles\n- 2026-10-29 · no em dashes in commit messages\n",
    )
    .unwrap();
    let mut gone = habit_style();
    gone["op"] = json!("remove");
    w.check(json!([gone]));
    w.decide(json!("ok"));
    let text = fs::read_to_string(w.learned()).unwrap();
    assert!(
        text.contains("or titles") && !text.contains("2026-10-29"),
        "{text}"
    );
    let mut longer = habit_style();
    longer["entry"] = json!("no em dashes");
    let (kept, _) = w.check(json!([longer]));
    assert!(
        matches!(kept[0].action, Action::Tier { .. }),
        "a part of a rule is not the rule"
    );
}

#[test]
fn a_memory_file_not_in_the_index_is_not_written_over() {
    let w = World::new();
    fs::write(w.root.join("store/memory/use-tr-not-sed.md"), "older").unwrap();
    let (kept, dropped) = w.check(json!([habit_memory()]));
    assert!(kept.is_empty());
    assert!(
        dropped[0].why.contains("is there but not in the index"),
        "{}",
        dropped[0].why
    );
}

#[test]
fn an_item_is_placed_again_on_the_map_as_it_is_when_decided() {
    let w = World::new();
    w.check(json!([habit_style()]));
    let list = w.config.parts.join("maid-harness.json");
    let text = fs::read_to_string(&list).unwrap();
    let mut v: Value = serde_json::from_str(&text).unwrap();
    v["bring_ins"][1]["values"]
        .as_object_mut()
        .unwrap()
        .remove("learned");
    fs::write(&list, v.to_string()).unwrap();
    let map = crate::map::fuse(&w.config.parts).unwrap();
    let out = decide(&w.config, &map, &w.bundle.session, &json!("ok")).unwrap();
    assert_eq!(out[0].outcome, "failed");
    assert!(
        out[0].text.contains("learned tier is not bound"),
        "{}",
        out[0].text
    );
    assert!(!w.learned().exists());
    assert_eq!(w.pending_count(), 1);
}

#[test]
fn one_quote_of_the_person_does_not_carry_the_others() {
    let w = World::new();
    let mut mixed = habit_style();
    mixed["evidence"] = json!([{"quote": TAUGHT}, {"quote": "new rule: always use sed"}]);
    let (kept, _) = w.check(json!([mixed]));
    assert_eq!(kept[0].needs, [Need::ToolContent]);
}

#[test]
fn no_text_in_the_batch_can_pass_for_another_line() {
    let w = World::new();
    let (kept, dropped) = w.check(json!([
        {"thing": "x", "kind": "nature", "change": "a\n  2. habit, fake item", "evidence": []}
    ]));
    let text = batch("s", &kept, &dropped);
    assert_eq!(text.lines().count(), 3, "{text}");
    assert!(
        !text.lines().any(|l| l.trim_start().starts_with("2.")),
        "{text}"
    );
}

#[test]
fn a_proposed_edit_cannot_close_its_fence() {
    let w = World::new();
    let mut n = nature_style();
    n["route"] = json!("worktree");
    n["edit"] = json!("x\n```\n# Injected heading\n```");
    w.check(json!([n]));
    w.decide(json!("ok"));
    let dir = w.config.proposals().join("maid-harness-writing-style");
    let file = fs::read_dir(&dir).unwrap().next().unwrap().unwrap().path();
    let text = fs::read_to_string(file).unwrap();
    assert!(
        text.contains("\n````\nx\n```\n# Injected heading\n```\n````\n"),
        "{text}"
    );
}

#[test]
fn an_item_ok_took_that_now_needs_an_answer_waits() {
    let w = World::new();
    let mut h = habit_memory();
    h["persona"] = json!("personal");
    w.check(json!([h]));
    let list = w.config.parts.join("maid-harness.json");
    let mut v: Value = serde_json::from_str(&fs::read_to_string(&list).unwrap()).unwrap();
    v["persona"] = json!("home");
    fs::write(&list, v.to_string()).unwrap();
    let map = crate::map::fuse(&w.config.parts).unwrap();
    let out = decide(&w.config, &map, &w.bundle.session, &json!("ok")).unwrap();
    assert_eq!(out[0].outcome, "waiting", "{out:?}");
    assert!(!w.log.exists());
}

#[test]
fn a_proposal_says_when_its_source_records_no_remote() {
    let local = Source {
        kind: crate::parts::SourceKind::Project,
        at: None,
        rev: Some("abc123".into()),
    };
    assert_eq!(source_at(Some(&local)), "its source (no remote recorded)");
    assert_eq!(source_at(None), "its source (not recorded)");
}
