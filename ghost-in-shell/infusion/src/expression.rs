//! expression: what this turn needs, as the prefix in front of the prompt.
//! Code gathers and the agent judges (D1): the essentials (acting persona,
//! its memory, the work's record, the first rules of each bring-in that
//! fits), and candidates that may bear on the turn for the model to open.
//! Each is its own item so only what changed is sent, and the essentials go
//! again in short once drifted (D2). Never the catalog. See the initiative
//! spec `harness-core`, "The five entry points" and "Who does what".
//!
//!   prefix          the order, the framing line, the budget, placement
//!   essentials      persona, record and fits, each with its full item and short entry
//!   reinforcement   the short form's text: heading, first rule lines, byte cap
//!   drift           when the short form is due again
//!   candidate       memories and near fits that may bear on the turn
//!   select          the memory indexes and the `Selector`
//!   fit             turn fit, near fits and the persona guess
//!   record, here    the work's own record; where the session is
//!   delivery        what each session was sent

pub mod candidate;
pub mod delivery;
pub mod drift;
pub mod essentials;
pub mod fit;
pub mod here;
pub mod prefix;
pub mod record;
pub mod reinforcement;
pub mod select;

use crate::call::{Call, Carrier, Infusion, Item};
use crate::expression::candidate::Net;
use crate::expression::delivery::Record;
use crate::expression::essentials::Essentials;
use crate::expression::here::Here;
use anyhow::Result;

/// What each item of the prefix begins with, so a reader (and the model)
/// can tell the ghost's text from the person's. The lines under an item
/// (a record's state, a candidate, a short-form entry) are its own.
pub const TAG: &str = "[ghost-in-shell]";
/// A record line or a memory's title and hook is cut to this.
pub const FIELD_BYTES: usize = 300;

/// The prefix for this turn: what changed of the essentials, their short
/// form once drifted, and the candidates. Reads the call's prompt and
/// folder.
pub fn expression(ctx: &Infusion, call: &Call) -> Result<Vec<Item>> {
    let here = Here::at(call.at.cwd.as_deref());
    let ghost = ctx.shell.ghost(here.cwd.as_deref())?;
    let prompt = call.prompt.as_deref().unwrap_or_default();
    // Whether this session was told the item before.
    let told = |id: &str| match (call.session.as_deref(), call.carrier, call.reset) {
        (Some(s), Carrier::Append, false) => Record::new(ctx.config.delivery())
            .sent_any(s, id)
            .unwrap_or(false),
        _ => false,
    };
    let essentials = Essentials::of(&ghost, &here, prompt, &told);
    let near: Vec<_> = essentials
        .past_cap
        .iter()
        .copied()
        .chain(fit::near(&ghost, prompt, &essentials.matched))
        .collect();
    let net = Net {
        acting: essentials.acting.as_deref(),
        fits: &essentials.fits,
        near: &near,
        widen: ctx.config.candidates,
    };
    let candidates = candidate::gather(&ghost, prompt, &net, &select::Lookup, &told);
    prefix::place(ctx, call, &essentials, &candidates)
}

/// `text` cut to at most `max` bytes at a character boundary, with control
/// characters other than newlines and tabs dropped, since it comes from
/// files the work owns and goes into a model's context.
pub(crate) fn clip(text: &str, max: usize) -> String {
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

/// `clip`, on one line: a field that must not start a line of its own,
/// such as a candidate's, under a framing line saying what it is.
pub(crate) fn one_line(text: &str, max: usize) -> String {
    clip(text, max).replace(['\n', '\r', '\t', '\u{2028}', '\u{2029}'], " ")
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::call::Where;
    use crate::expression::delivery::deliver_within;
    use crate::expression::essentials::{RULES_BYTES, RULES_LINES};
    use crate::expression::prefix::TURN_BYTES;
    use crate::testing::{call, ctx};
    use serde_json::{json, Value};
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    pub(crate) fn part(dir: &Path, name: &str, persona: &str, bring_ins: Value) {
        let parts = dir.join("parts");
        fs::create_dir_all(&parts).unwrap();
        let v = json!({"format": 1, "part": name, "harnesses": {}, "persona": persona, "bring_ins": bring_ins});
        fs::write(parts.join(format!("{name}.json")), v.to_string()).unwrap();
    }

    /// A part with a turn skill `name` fitting the job `job`, whose rules
    /// section says `rule`, and a memory whose index holds `memories`.
    pub(crate) fn turn_part(
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
            fs::write(&unit, format!("---\nname: {name}\ndescription: d\nmetadata:\n  ghost-in-shell.contract: \"1\"\n  ghost-in-shell.scope: turn\n  ghost-in-shell.job: {name} work\n  ghost-in-shell.signals.jobs: {job}\n  ghost-in-shell.rules: SKILL.md#rules\n---\n## Rules\n\n{rule}\n\n## Other\n\nnot sent\n")).unwrap();
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
            json!({"name": "m", "_meta": {"ghost-in-shell": decl}}).to_string(),
        )
        .unwrap();
        bring_ins.push(json!({"name": format!("{part_name}-memory"), "unit": {"standard": "mcp", "path": server},
            "values": {"store": store, "index": "memory/MEMORY.md"}}));
        part(dir, part_name, persona, Value::Array(bring_ins));
    }

    fn said(c: &Infusion, carrier: Carrier, prompt: &str) -> Vec<String> {
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
        assert_eq!(first[1], candidate::FRAMING, "{first:?}");
        assert!(first[2].starts_with("recall."), "{first:?}");
        assert_eq!(first.len(), 3);
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
        assert_eq!(said(&c, Carrier::Transient, "deploy the release").len(), 3);
        assert_eq!(said(&c, Carrier::OnDemand, "deploy the release").len(), 3);
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
        fs::write(&unit, "---\nname: kit\ndescription: d\nmetadata:\n  ghost-in-shell.contract: \"1\"\n  ghost-in-shell.scope: turn\n  ghost-in-shell.job: specs\n  ghost-in-shell.signals.files: specs/feature/*.md\n  ghost-in-shell.record: specs/feature/*.md#handoff\n---\n").unwrap();
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
                "[ghost-in-shell] persona: none known here\n",
                "[ghost-in-shell] record: none here\n"
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
        fs::write(&unit, "---\nname: garden\ndescription: d\nmetadata:\n  ghost-in-shell.contract: \"1\"\n  ghost-in-shell.scope: turn\n  ghost-in-shell.job: plan the beds\n  ghost-in-shell.signals.locations: garden-*\n---\n").unwrap();
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
        assert_eq!(texts, ["[ghost-in-shell] persona: none known here\n"]);
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
    fn a_bring_in_that_fits_past_the_cap_may_apply() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        turn_part(
            dir.path(),
            "a",
            "personal",
            &[
                ("one", "deploy", "ONE"),
                ("two", "deploy", "TWO"),
                ("three", "deploy", "THREE"),
                ("four", "deploy", "FOUR"),
            ],
            "",
        );
        let call = Call {
            prompt: Some("deploy".into()),
            ..call(None, Carrier::Transient, false)
        };
        let items = expression(&c, &call).unwrap();
        let near: Vec<&str> = items
            .iter()
            .filter(|i| i.id.starts_with("near."))
            .map(|i| i.text.as_str())
            .collect();
        assert_eq!(near, ["- may apply: four (four work)\n"]);
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
