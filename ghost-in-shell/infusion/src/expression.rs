//! expression: what this turn needs, each as its own item so only what
//! changed is sent: the reinforcement (acting persona, its memory, the
//! work's record), the first rules of each bring-in that fits, and the
//! memories that may bear on it. Never the catalog. See the initiative
//! spec `harness-core`, "The five entry points".

pub mod delivery;
pub mod fit;
pub mod here;
pub mod record;
pub mod select;

use crate::call::{Call, Carrier, Ctx, Item};
use crate::expression::delivery::{deliver_within, Record};
use crate::expression::here::Here;
use crate::expression::select::Selector;
use anyhow::Result;
use ghost::contract::{self, Scope};
use ghost::{BringIn, Ghost};
use std::fmt::Write as _;

/// Claude Code places a hook's `additionalContext` whole only up to 10,000
/// characters; a turn's items stay under this many bytes, which leaves room
/// for the framing, and a byte count is never below the character count.
const TURN_BYTES: usize = 9_000;
/// No one item may take most of a turn: every piece of text an item takes
/// from a file is cut to these.
const RULES_LINES: usize = 30;
const RULES_BYTES: usize = 2_000;
const FIELD_BYTES: usize = 300;

/// What this turn needs, each as its own item so only what changed is
/// sent: the reinforcement (acting persona, its memory, the work's record),
/// the first rules of each bring-in that fits, and the memories that bear
/// on it. Never the catalog. Reads the call's prompt and folder.
pub fn expression(ctx: &Ctx, call: &Call) -> Result<Vec<Item>> {
    let here = Here::at(call.at.cwd.as_deref());
    let ghost = ctx.shell.ghost(here.cwd.as_deref())?;
    let prompt = call.prompt.as_deref().unwrap_or_default();
    let acting = fit::persona(&ghost, &here);
    let record = work_record(&ghost, &here);
    // Whether this session was told the item before, so that leaving what
    // it says is said once, and only to a session that was told it.
    let told = |id: &str| match (call.session.as_deref(), call.carrier, call.reset) {
        (Some(s), Carrier::Append, false) => Record::new(ctx.config.delivery())
            .sent_any(s, id)
            .unwrap_or(false),
        _ => false,
    };
    let mut items: Vec<Item> = [
        reinforce_persona(&ghost, acting.as_deref(), told(PERSONA)),
        reinforce_record(record.as_ref(), told(RECORD)),
    ]
    .into_iter()
    .flatten()
    .collect();
    let state = record.as_ref().and_then(|r| r.state.as_deref());
    let mut seen_rules = std::collections::BTreeSet::new();
    let mut fitted = 0;
    for f in fit::ranked(&ghost, prompt, state, &here) {
        if fitted == fit::CAP {
            break;
        }
        let b = f.bring_in;
        let rules = rules_of(b);
        if let Ok(Some(text)) = &rules {
            if !seen_rules.insert(text.clone()) {
                continue;
            }
        }
        // The cap counts fits sent, so rules two parts share do not use it.
        fitted += 1;
        let mut text = format!(
            "[my-harness] fits this turn: {} ({}): {}\n",
            clip(&b.name, FIELD_BYTES),
            clip(b.persona.as_deref().unwrap_or(&b.part), FIELD_BYTES),
            clip(b.job().unwrap_or("no job given"), FIELD_BYTES)
        );
        if let Ok(Some(r)) = &rules {
            text.push_str(r);
            text.push('\n');
        }
        items.push(Item {
            id: fit_id(&b.part, &b.name),
            text,
            // Only what the bring-in is, not why this prompt fit it: the
            // record hashes the data too, and the same rules must not be sent
            // again because another word matched.
            data: serde_json::json!({"part": b.part, "name": b.name,
                "rules": rules.as_ref().err()}),
        });
    }
    let (memories, _) = select::memories(&ghost, acting.as_deref());
    let query = select::Query { prompt };
    for m in select::Lookup.select(&query, &memories, select::CAP) {
        items.push(Item {
            id: format!(
                "recall.{:016x}",
                delivery::fnv1a(&m.file.display().to_string())
            ),
            text: format!(
                "[my-harness] memory ({}): [{}]({}) - {}\n",
                m.persona.as_deref().unwrap_or(&m.bring_in),
                clip(&m.title, FIELD_BYTES),
                m.file.display(),
                clip(&m.hook, FIELD_BYTES)
            ),
            data: serde_json::to_value(&m)?,
        });
    }
    deliver_within(ctx, call, items, TURN_BYTES)
}

/// The work's own record: the first turn bring-in whose `record` names one
/// here, whether or not it fits this turn, since its state is part of what
/// decides the fit.
fn work_record(ghost: &Ghost, here: &Here) -> Option<record::Record> {
    ghost
        .bring_ins
        .iter()
        .filter_map(|b| b.declaration())
        .filter(|d| d.scope == Scope::Turn && !d.record.is_empty())
        .find_map(|d| record::find(&d.record, here).ok().flatten())
}

const PERSONA: &str = "reinforce.persona";
const RECORD: &str = "reinforce.record";

/// The acting persona and its memory, restated only where it can change:
/// with one persona the genome has said all there is. Where none is known,
/// one line, and only to a session that was told one.
fn reinforce_persona(ghost: &Ghost, acting: Option<&str>, told: bool) -> Option<Item> {
    let personas: std::collections::BTreeSet<&str> = ghost
        .personas
        .iter()
        .filter_map(|p| p.persona.as_deref())
        .collect();
    if personas.len() < 2 {
        return None;
    }
    let text = match acting {
        Some(persona) => {
            let memory: Vec<String> = ghost
                .bring_ins
                .iter()
                .filter(|b| b.persona.as_deref() == Some(persona))
                .filter(|b| b.declaration().is_some_and(|d| d.scope == Scope::Session))
                .filter_map(|b| {
                    let l = crate::genome::line(b);
                    (!l.state.is_empty())
                        .then(|| clip(&format!("{} ({})", l.name, l.state.join(", ")), FIELD_BYTES))
                })
                .take(10)
                .collect();
            match memory.is_empty() {
                true => format!("[my-harness] acting through {persona}; no memory of its own\n"),
                false => format!(
                    "[my-harness] acting through {persona}; its memory: {}\n",
                    memory.join(", ")
                ),
            }
        }
        None if told => "[my-harness] persona: none known here\n".into(),
        None => return None,
    };
    Some(Item {
        id: PERSONA.into(),
        text,
        data: serde_json::json!({ "acting": acting }),
    })
}

/// The work's record: goal, state and next. Where there is none, one line,
/// and only to a session that was told one.
fn reinforce_record(record: Option<&record::Record>, told: bool) -> Option<Item> {
    let text = match record {
        Some(r) => {
            let mut text = format!(
                "[my-harness] the work's record: {}\n",
                clip(&r.file.display().to_string(), FIELD_BYTES)
            );
            for (label, v) in [("goal", &r.goal), ("state", &r.state), ("next", &r.next)] {
                if let Some(v) = v {
                    let _ = writeln!(text, "  {label}: {}", clip(v, FIELD_BYTES));
                }
            }
            text
        }
        None if told => "[my-harness] record: none here\n".into(),
        None => return None,
    };
    Some(Item {
        id: RECORD.into(),
        text,
        data: serde_json::json!({ "record": record }),
    })
}

/// A bring-in's first rules, read from its unit through its `rules` pointer.
fn rules_of(b: &BringIn) -> std::result::Result<Option<String>, String> {
    let (Some(d), Some(unit)) = (
        b.declaration(),
        b.record.as_ref().and_then(|r| r.unit.as_ref()),
    ) else {
        return Ok(None);
    };
    match &d.rules {
        None => Ok(None),
        Some(pointer) => contract::rules(&unit.path, pointer).map(|r| {
            let lines: Vec<&str> = r.lines().take(RULES_LINES).collect();
            Some(clip(&lines.join("\n"), RULES_BYTES))
        }),
    }
}

/// `text` cut to at most `max` bytes at a character boundary, with control
/// characters other than newlines and tabs dropped, since it comes from
/// files the work owns and goes into a model's context.
fn clip(text: &str, max: usize) -> String {
    let clean: String = text
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect();
    if clean.len() <= max {
        return clean;
    }
    let mut end = max.saturating_sub(3);
    while !clean.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &clean[..end])
}

/// A fit's item id: its part and name, or a hash of them when either is
/// not plain enough or long enough to be an id as it is, so two names
/// never share one.
fn fit_id(part: &str, name: &str) -> String {
    let id = format!("fit.{part}.{name}");
    let plain = |x: &str| {
        x.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_".contains(c))
    };
    match plain(part) && plain(name) && delivery::check_id("item", &id).is_ok() {
        true => id,
        false => format!("fit.{:016x}", delivery::fnv1a(&format!("{part}\0{name}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call::Where;
    use crate::testing::{call, ctx};
    use serde_json::{json, Value};
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn part(dir: &Path, name: &str, persona: &str, bring_ins: Value) {
        let parts = dir.join("parts");
        fs::create_dir_all(&parts).unwrap();
        let v = json!({"format": 1, "part": name, "harnesses": {}, "persona": persona, "bring_ins": bring_ins});
        fs::write(parts.join(format!("{name}.json")), v.to_string()).unwrap();
    }

    /// A part with a turn skill `name` fitting the job `job`, whose rules
    /// section says `rule`, and a memory whose index holds `memories`.
    fn turn_part(
        dir: &Path,
        part_name: &str,
        persona: &str,
        skills: &[(&str, &str, &str)],
        memories: &str,
    ) {
        let mut bring_ins = vec![];
        for (name, job, rule) in skills {
            let unit = dir.join(format!("units/{part_name}/{name}/SKILL.md"));
            fs::create_dir_all(unit.parent().unwrap()).unwrap();
            fs::write(&unit, format!("---\nname: {name}\ndescription: d\nmetadata:\n  my-harness.contract: \"1\"\n  my-harness.scope: turn\n  my-harness.job: {name} work\n  my-harness.signals.jobs: {job}\n  my-harness.rules: SKILL.md#rules\n---\n## Rules\n\n{rule}\n\n## Other\n\nnot sent\n")).unwrap();
            bring_ins.push(json!({"name": name, "unit": {"standard": "skill", "path": unit}}));
        }
        let store = dir.join(format!("{part_name}-store"));
        fs::create_dir_all(store.join("memory")).unwrap();
        fs::write(store.join("memory/MEMORY.md"), memories).unwrap();
        let server = dir.join("server.json");
        let decl = json!({"contract": 1, "scope": "session", "job": "memory",
            "config": {"store": {"about": "s"}, "index": {"about": "i"}},
            "state": {"permanent": "store", "promote": "off"}});
        fs::write(
            &server,
            json!({"name": "m", "_meta": {"my-harness": decl}}).to_string(),
        )
        .unwrap();
        bring_ins.push(json!({"name": format!("{part_name}-memory"), "unit": {"standard": "mcp", "path": server},
            "values": {"store": store, "index": "memory/MEMORY.md"}}));
        part(dir, part_name, persona, Value::Array(bring_ins));
    }

    fn said(c: &Ctx, carrier: Carrier, prompt: &str) -> Vec<String> {
        let call = Call {
            prompt: Some(prompt.into()),
            at: Where {
                cwd: Some("/".into()),
                ..Where::default()
            },
            ..call(Some("s"), carrier, false)
        };
        expression(c, &call)
            .unwrap()
            .into_iter()
            .map(|i| i.id)
            .collect()
    }
    #[test]
    fn with_no_parts_every_entry_point_answers_nothing() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        let k = call(Some("s"), Carrier::Append, false);
        assert!(expression(&c, &k).unwrap().is_empty());
    }
    #[test]
    fn expression_sends_what_fits_once_and_only_what_changed() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        turn_part(
            dir.path(),
            "home",
            "personal",
            &[
                ("deployer", "deploy a release", "ALWAYS TAG FIRST"),
                ("writer", "draft an email", "SHORT LINES"),
            ],
            "- [Deploy steps](deploy.md) - how a release is deployed\n",
        );
        let first = said(&c, Carrier::Append, "deploy the release");
        assert_eq!(
            first[0], "fit.home.deployer",
            "one persona: nothing to restate"
        );
        assert!(first[1].starts_with("recall."), "{first:?}");
        assert_eq!(first.len(), 2);
        assert!(
            said(&c, Carrier::Append, "deploy the release").is_empty(),
            "nothing changed"
        );
        assert_eq!(
            said(
                &c,
                Carrier::Append,
                "deploy the release, then draft an email"
            ),
            ["fit.home.writer"],
            "only the new fit"
        );
        assert!(
            said(&c, Carrier::Append, "hello").is_empty(),
            "nothing fits: nothing"
        );
        assert_eq!(said(&c, Carrier::Transient, "deploy the release").len(), 2);
        assert_eq!(said(&c, Carrier::OnDemand, "deploy the release").len(), 2);
    }
    #[test]
    fn a_fit_carries_its_rules_section_and_the_same_rules_from_two_parts_are_sent_once() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        turn_part(
            dir.path(),
            "a",
            "personal",
            &[("deployer", "deploy", "ALWAYS TAG FIRST")],
            "",
        );
        turn_part(
            dir.path(),
            "b",
            "personal",
            &[("deployer", "deploy", "ALWAYS TAG FIRST")],
            "",
        );
        let call = Call {
            prompt: Some("deploy".into()),
            ..call(None, Carrier::Transient, false)
        };
        let items = expression(&c, &call).unwrap();
        let fits: Vec<&Item> = items.iter().filter(|i| i.id.starts_with("fit.")).collect();
        assert_eq!(fits.len(), 1, "{items:?}");
        assert!(
            fits[0].text.contains("## Rules\n\nALWAYS TAG FIRST"),
            "{}",
            fits[0].text
        );
        assert!(!fits[0].text.contains("not sent"));
    }
    #[test]
    fn the_reinforcement_follows_the_acting_persona_and_the_record() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        turn_part(dir.path(), "home", "personal", &[], "");
        let repo = dir.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::write(repo.join(".git/HEAD"), "ref: refs/heads/feat/hooks/work\n").unwrap();
        fs::create_dir_all(repo.join("specs/feature")).unwrap();
        fs::write(
            repo.join("specs/feature/hooks.md"),
            "# Feature: hooks\n\n## Handoff\n\n- **Stage:** dev\n- **Ready for:** review\n",
        )
        .unwrap();
        let unit = dir.path().join("units/kit/SKILL.md");
        fs::create_dir_all(unit.parent().unwrap()).unwrap();
        fs::write(&unit, "---\nname: kit\ndescription: d\nmetadata:\n  my-harness.contract: \"1\"\n  my-harness.scope: turn\n  my-harness.job: specs\n  my-harness.signals.files: specs/feature/*.md\n  my-harness.record: specs/feature/*.md#handoff\n---\n").unwrap();
        let parts = dir.path().join("parts/home.json");
        let mut v: Value = serde_json::from_str(&fs::read_to_string(&parts).unwrap()).unwrap();
        v["bring_ins"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name": "kit", "unit": {"standard": "skill", "path": unit}}));
        fs::write(&parts, v.to_string()).unwrap();
        let at = |cwd: &Path| Call {
            prompt: Some("go on".into()),
            at: Where {
                cwd: Some(cwd.into()),
                ..Where::default()
            },
            ..call(Some("s"), Carrier::Append, false)
        };
        let alone = expression(&c, &at(&repo)).unwrap();
        assert_eq!(alone[0].id, "reinforce.record");
        assert!(
            !alone.iter().any(|i| i.text.contains("acting through")),
            "one persona: the genome said it"
        );
        assert!(
            alone[0].text.contains("goal: Feature: hooks"),
            "{}",
            alone[0].text
        );
        part(dir.path(), "job", "work", json!([]));
        let c = ctx(dir.path());
        let at = |cwd: &Path| Call {
            prompt: Some("go on".into()),
            at: Where {
                cwd: Some(cwd.into()),
                ..Where::default()
            },
            ..call(Some("t"), Carrier::Append, false)
        };
        let items = expression(&c, &at(&repo)).unwrap();
        assert_eq!(items[0].id, "reinforce.persona");
        let text = &items[0].text;
        assert!(
            text.contains("acting through personal; its memory: home-memory ("),
            "two personas, this location points at one: {text}"
        );
        let text = &items[1].text;
        assert!(
            text.contains("goal: Feature: hooks\n  state: dev\n  next: review"),
            "{text}"
        );
        assert!(expression(&c, &at(&repo)).unwrap().is_empty());
        let left: Vec<String> = expression(&c, &at(dir.path()))
            .unwrap()
            .into_iter()
            .map(|i| i.text)
            .collect();
        assert_eq!(
            left,
            [
                "[my-harness] persona: none known here\n",
                "[my-harness] record: none here\n"
            ],
            "leaving the location is said once"
        );
        assert!(expression(&c, &at(dir.path())).unwrap().is_empty());
        let fresh = Call {
            prompt: Some("go on".into()),
            at: Where {
                cwd: Some(dir.path().into()),
                ..Where::default()
            },
            ..call(Some("fresh"), Carrier::Append, false)
        };
        assert!(
            expression(&c, &fresh).unwrap().is_empty(),
            "a session never told a persona or record is told nothing"
        );
    }
    #[test]
    fn a_session_told_a_persona_and_never_a_record_is_never_told_none_here() {
        let dir = TempDir::new().unwrap();
        turn_part(dir.path(), "home", "personal", &[], "");
        part(dir.path(), "job", "work", json!([]));
        let unit = dir.path().join("units/garden/SKILL.md");
        fs::create_dir_all(unit.parent().unwrap()).unwrap();
        fs::write(&unit, "---\nname: garden\ndescription: d\nmetadata:\n  my-harness.contract: \"1\"\n  my-harness.scope: turn\n  my-harness.job: plan the beds\n  my-harness.signals.locations: garden-*\n---\n").unwrap();
        let parts = dir.path().join("parts/home.json");
        let mut v: Value = serde_json::from_str(&fs::read_to_string(&parts).unwrap()).unwrap();
        v["bring_ins"]
            .as_array_mut()
            .unwrap()
            .push(json!({"name": "garden", "unit": {"standard": "skill", "path": unit}}));
        fs::write(&parts, v.to_string()).unwrap();
        let garden = dir.path().join("garden-planner");
        fs::create_dir_all(&garden).unwrap();
        let c = ctx(dir.path());
        let at = |cwd: &Path, prompt: &str| {
            let call = Call {
                prompt: Some(prompt.into()),
                at: Where {
                    cwd: Some(cwd.into()),
                    ..Where::default()
                },
                ..call(Some("s"), Carrier::Append, false)
            };
            expression(&c, &call).unwrap()
        };
        let first = at(&garden, "what goes in the beds");
        assert_eq!(first[0].id, "reinforce.persona", "{first:?}");
        assert!(first[0].text.contains("acting through personal"));
        assert!(
            !first.iter().any(|i| i.text.contains("record:")),
            "{first:?}"
        );
        assert!(
            at(&garden, "what goes in the beds").is_empty(),
            "a second prompt with nothing changed gets nothing"
        );
        let left = at(dir.path(), "and elsewhere");
        let texts: Vec<&str> = left.iter().map(|i| i.text.as_str()).collect();
        assert_eq!(texts, ["[my-harness] persona: none known here\n"]);
    }
    #[test]
    fn the_turn_budget_counts_only_what_is_sent_and_no_one_item_takes_it() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        let skills: Vec<(String, String, String)> = (0..5)
            .map(|i| {
                let rule = format!("RULE {i} {}\n", "TAG FIRST ".repeat(40)).repeat(40);
                (format!("big{i}"), format!("deploy{i}"), rule)
            })
            .collect();
        let spec: Vec<(&str, &str, &str)> = skills
            .iter()
            .map(|(n, j, r)| (n.as_str(), j.as_str(), r.as_str()))
            .collect();
        turn_part(dir.path(), "home", "personal", &spec, "");
        let one = |prompt: &str| {
            let call = Call {
                prompt: Some(prompt.into()),
                ..call(Some("s"), Carrier::Append, false)
            };
            expression(&c, &call).unwrap()
        };
        let first = one("deploy0 deploy1 deploy2");
        for i in &first {
            assert!(
                i.text.len() <= RULES_BYTES + 200,
                "{} is {} bytes",
                i.id,
                i.text.len()
            );
            assert!(i.text.lines().count() <= RULES_LINES + 2, "{}", i.id);
        }
        assert!(first.iter().map(|i| i.text.len()).sum::<usize>() <= TURN_BYTES);
        assert_eq!(first.len(), 3, "all three fits");
        let second = one("deploy0 deploy1 deploy2");
        assert!(second.is_empty(), "already sent: nothing");
        let third = one("deploy3 deploy4 deploy0");
        let ids: Vec<&str> = third.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            ids,
            ["fit.home.big3", "fit.home.big4"],
            "what was sent takes no budget"
        );
    }
    #[test]
    fn the_budget_leaves_out_what_the_session_holds_before_it_trims() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        let item = |id: &str| Item {
            id: id.into(),
            text: "x".repeat(60),
            data: Value::Null,
        };
        let append = call(Some("s"), Carrier::Append, false);
        let sent = deliver_within(&c, &append, vec![item("a")], 100).unwrap();
        assert_eq!(sent.len(), 1);
        let next = deliver_within(&c, &append, vec![item("a"), item("b")], 100).unwrap();
        let ids: Vec<&str> = next.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["b"], "the held item takes no budget");
        let over = deliver_within(&c, &append, vec![item("c"), item("d")], 100).unwrap();
        assert_eq!(over.len(), 1, "the second is over the budget");
        let later = deliver_within(&c, &append, vec![item("d")], 100).unwrap();
        assert_eq!(later.len(), 1, "and was not recorded, so it comes later");
    }
    #[test]
    fn the_same_rules_are_not_sent_again_when_another_word_fits_them() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        turn_part(
            dir.path(),
            "home",
            "personal",
            &[("deployer", "deploy, release", "TAG FIRST")],
            "",
        );
        assert!(said(&c, Carrier::Append, "deploy it").contains(&"fit.home.deployer".to_string()));
        assert!(said(&c, Carrier::Append, "release it").is_empty());
    }
    #[test]
    fn a_name_that_is_not_a_plain_id_is_hashed_and_never_shared() {
        assert_eq!(fit_id("home", "notes"), "fit.home.notes");
        let (a, b) = (fit_id("home", "a.b"), fit_id("home", "a-b"));
        assert!(a.starts_with("fit.") && a != b, "{a} {b}");
        assert!(delivery::check_id("item", &fit_id("p", &"x".repeat(200))).is_ok());
    }
    #[test]
    fn rules_two_parts_share_do_not_use_up_the_cap() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        turn_part(
            dir.path(),
            "a",
            "personal",
            &[
                ("one", "deploy", "SAME"),
                ("two", "deploy", "TWO"),
                ("three", "deploy", "THREE"),
            ],
            "",
        );
        turn_part(
            dir.path(),
            "b",
            "personal",
            &[("one", "deploy", "SAME")],
            "",
        );
        let call = Call {
            prompt: Some("deploy".into()),
            ..call(None, Carrier::Transient, false)
        };
        let fits = expression(&c, &call)
            .unwrap()
            .into_iter()
            .filter(|i| i.id.starts_with("fit."))
            .count();
        assert_eq!(fits, 3);
    }
}
