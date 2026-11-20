//! The prefix: the text in front of a prompt. In order, the essentials that
//! changed (in full), their short form when it is due (`Reinforcement`,
//! timed by `Drift`), and the candidates under their framing line; at most
//! `TURN_BYTES` in all, and only what the session does not already hold.
//! A second prompt with nothing changed and nothing drifted gets nothing.
//! See the initiative spec `harness-core`, "On Claude Code" (the prefix),
//! and project.md, "Expression".

use crate::call::{Call, Carrier, Infusion, Item};
use crate::experience::source::Located;
use crate::expression::candidate::{self, Candidate, FRAMING};
use crate::expression::delivery::{self, Record};
use crate::expression::drift::{self, Drift};
use crate::expression::essentials::Essentials;
use crate::expression::reinforcement;
use anyhow::Result;
use std::collections::BTreeSet;

/// Claude Code places a hook's `additionalContext` whole only up to 10,000
/// characters; a turn stays under this many bytes, which leaves room for
/// the framing, and a byte count is never below the character count.
pub const TURN_BYTES: usize = 9_000;
/// The short form's item. It is no mark in the record: drift decides when
/// it goes, not whether its text was seen.
pub const SHORT: &str = "reinforce.short";

/// The items this call places for `essentials` and `candidates`, and, on
/// an append carrier with a session, what the session's drift state
/// becomes.
pub fn place(
    ctx: &Infusion,
    call: &Call,
    essentials: &Essentials,
    candidates: &[Candidate],
) -> Result<Vec<Item>> {
    let offered: Vec<Item> = candidates.iter().map(Candidate::item).collect();
    let offered_ids: BTreeSet<String> = offered.iter().map(|i| i.id.clone()).collect();
    let (fresh_candidates, fresh_full): (Vec<Item>, Vec<Item>) =
        delivery::unheld(ctx, call, [essentials.items(), offered].concat())?
            .into_iter()
            .partition(|i| offered_ids.contains(&i.id));

    let record = Record::new(ctx.config.delivery());
    let session = match (call.session.as_deref(), call.carrier, &call.prompt) {
        (Some(s), Carrier::Append, Some(_)) => Some(s),
        _ => None,
    };
    let drift = match session {
        Some(s) => Some(Drift::read(record.state(s, drift::FILE)?.as_deref()).next(
            conversation_size(ctx, call),
            call.prompt.as_deref().map_or(0, |p| p.len() as u64),
        )),
        None => None,
    };
    let fits = essentials.fit_ids();
    let full: BTreeSet<&str> = fresh_full.iter().map(|i| i.id.as_str()).collect();
    let shorts: Vec<reinforcement::Short> = match &drift {
        None => vec![],
        Some(d) => {
            let returning = d.returning(&fits);
            essentials
                .all
                .iter()
                .filter_map(|e| e.short.as_ref())
                .filter(|s| !full.contains(s.id.as_str()))
                .filter(|s| d.drifted() || returning.contains(&s.id))
                .cloned()
                .collect()
        }
    };

    let mut items = fresh_full;
    if let Some(text) = reinforcement::text(&shorts) {
        let ids: Vec<&str> = shorts.iter().map(|s| s.id.as_str()).collect();
        items.push(Item {
            id: SHORT.into(),
            text,
            data: serde_json::json!({ "for": ids }),
        });
    }
    if !fresh_candidates.is_empty() {
        items.push(candidate::framing());
        items.extend(fresh_candidates);
    }
    let mut sent = vec![];
    for item in delivery::within(items, TURN_BYTES) {
        match item.id == SHORT || item.id == FRAMING {
            true => sent.push(item),
            false => sent.extend(delivery::claim(ctx, call, vec![item])?),
        }
    }
    let sent = framed(sent);

    if let (Some(s), Some(d)) = (session, drift) {
        let sent_ids: BTreeSet<&str> = sent.iter().map(|i| i.id.as_str()).collect();
        let whole = placed_whole(essentials, &d, &sent_ids);
        record.set_state(s, drift::FILE, &d.after(fits, whole).write())?;
    }
    Ok(sent)
}

/// `sent` without a framing line that ended up with no candidate under it:
/// the budget cut them, or another carrier placed them first.
fn framed(mut sent: Vec<Item>) -> Vec<Item> {
    if sent.last().is_some_and(|i| i.id == FRAMING) {
        sent.pop();
    }
    sent
}

/// Whether this turn placed the essentials whole: the short form went
/// because they had drifted, or every one with a full item went in full,
/// or these are the session's first. The second moves the mark on a turn
/// where nothing drifted but everything changed, since all of it is now
/// recent.
fn placed_whole(essentials: &Essentials, d: &Drift, sent: &BTreeSet<&str>) -> bool {
    if essentials.all.is_empty() {
        return false;
    }
    let full: Vec<&str> = essentials
        .all
        .iter()
        .filter_map(|e| e.full.as_ref().map(|f| f.id.as_str()))
        .collect();
    (sent.contains(SHORT) && d.drifted())
        || d.placed.is_none()
        || (!full.is_empty() && full.iter().all(|id| sent.contains(id)))
}

/// How many bytes of conversation the session holds, where the call names
/// its file and the session source can read it as this session's: the
/// source decides which of the file's entries are conversation, since a
/// shell writes its own bookkeeping there too. Sub-agents' files are not
/// read. `None` otherwise, and drift then counts the prompts' bytes.
fn conversation_size(ctx: &Infusion, call: &Call) -> Option<u64> {
    call.at.transcript.as_ref()?;
    let source = ctx.sources.named(call.at.source.as_deref()).ok()?;
    let at = Located {
        subs: vec![],
        ..source.locate(call.session.as_deref(), &call.at).ok()?
    };
    Some(source.pack(&at).ok()?.conversation_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call::Where;
    use crate::expression::expression;
    use crate::expression::tests::turn_part;
    use crate::testing::{call, ctx};
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    /// A ghost with one persona, a `deployer` and a `writer` skill and a
    /// memory holding one deploy memory.
    fn world(dir: &Path) {
        turn_part(
            dir,
            "home",
            "personal",
            &[
                (
                    "deployer",
                    "deploy a release",
                    "- TAG FIRST\n- THEN DEPLOY\n- THIRD",
                ),
                ("writer", "draft an email", "SHORT LINES"),
            ],
            "- [Deploy steps](deploy.md) - how a release is deployed\n",
        );
    }

    fn turn(c: &Infusion, session: &str, prompt: &str, transcript: Option<&Path>) -> Vec<Item> {
        let call = Call {
            prompt: Some(prompt.into()),
            at: Where {
                transcript: transcript.map(Path::to_path_buf),
                ..Where::default()
            },
            ..call(Some(session), Carrier::Append, false)
        };
        expression(c, &call).unwrap()
    }

    fn ids(items: &[Item]) -> Vec<&str> {
        items.iter().map(|i| i.id.as_str()).collect()
    }

    #[test]
    fn a_steady_session_gets_nothing_until_ten_prompts_then_the_short_form_once() {
        let dir = TempDir::new().unwrap();
        world(dir.path());
        let c = ctx(dir.path());
        assert!(ids(&turn(&c, "s", "deploy the release", None)).contains(&"fit.home.deployer"));
        for n in 2..=10 {
            assert!(
                turn(&c, "s", "deploy the release", None).is_empty(),
                "prompt {n}"
            );
        }
        let eleventh = turn(&c, "s", "deploy the release", None);
        assert_eq!(ids(&eleventh), [SHORT]);
        let text = &eleventh[0].text;
        assert!(
            text.contains("- acting through personal; its memory: home-memory ("),
            "{text}"
        );
        assert!(
            text.contains("- deployer:\n    TAG FIRST\n    THEN DEPLOY\n"),
            "{text}"
        );
        assert!(!text.contains("THIRD"), "the first two rule lines only");
        assert!(
            turn(&c, "s", "deploy the release", None).is_empty(),
            "placed again: nothing"
        );
    }

    #[test]
    fn the_conversation_grown_two_hundred_kb_gets_the_short_form() {
        let dir = TempDir::new().unwrap();
        world(dir.path());
        let c = Infusion {
            sources: Box::new(crate::testing::SaidLines),
            ..ctx(dir.path())
        };
        let t = dir.path().join("t.jsonl");
        let said = |n: usize| format!("said {}\n", "x".repeat(n));
        let mut file = said(1_000);
        fs::write(&t, &file).unwrap();
        turn(&c, "s", "deploy the release", Some(&t));
        file += &format!("{}\n", "b".repeat(240_000));
        fs::write(&t, &file).unwrap();
        assert!(
            turn(&c, "s", "deploy the release", Some(&t)).is_empty(),
            "240 KB the source does not count as conversation: nothing"
        );
        file += &said(drift::BYTES as usize - 1);
        fs::write(&t, &file).unwrap();
        assert!(
            turn(&c, "s", "deploy the release", Some(&t)).is_empty(),
            "a byte under 200 KB of conversation"
        );
        file += &said(1);
        fs::write(&t, &file).unwrap();
        assert_eq!(ids(&turn(&c, "s", "deploy the release", Some(&t))), [SHORT]);
        assert!(turn(&c, "s", "deploy the release", Some(&t)).is_empty());
    }

    #[test]
    fn a_fit_that_returns_gets_its_short_form_alone() {
        let dir = TempDir::new().unwrap();
        world(dir.path());
        let c = ctx(dir.path());
        turn(&c, "s", "deploy the release", None);
        assert_eq!(
            ids(&turn(&c, "s", "draft an email", None)),
            ["fit.home.writer"]
        );
        let back = turn(&c, "s", "deploy the release", None);
        assert_eq!(ids(&back), [SHORT]);
        assert!(back[0].text.contains("- deployer:"));
        assert!(
            !back[0].text.contains("acting through"),
            "only what returned"
        );
        assert!(turn(&c, "s", "deploy the release", None).is_empty());
    }

    #[test]
    fn a_fit_set_all_new_and_sent_in_full_moves_the_mark() {
        let dir = TempDir::new().unwrap();
        world(dir.path());
        let c = ctx(dir.path());
        turn(&c, "s", "deploy the release", None);
        for _ in 2..=5 {
            turn(&c, "s", "deploy the release", None);
        }
        let sixth = turn(&c, "s", "draft an email", None);
        assert_eq!(ids(&sixth)[0], "fit.home.writer", "{sixth:?}");
        for n in 7..=15 {
            assert!(
                turn(&c, "s", "draft an email", None).is_empty(),
                "prompt {n}: the mark moved to prompt 6"
            );
        }
        assert_eq!(ids(&turn(&c, "s", "draft an email", None)), [SHORT]);
    }

    #[test]
    fn the_turn_budget_counts_the_framing_line_and_cuts_candidates_last() {
        let dir = TempDir::new().unwrap();
        let rules: Vec<String> = (0..3)
            .map(|i| format!("RULE {i} {}\n", "TAG FIRST ".repeat(40)).repeat(40))
            .collect();
        let skills: Vec<(String, String)> = (0..3)
            .map(|i| (format!("big{i}"), format!("deploy{i}")))
            .collect();
        let spec: Vec<(&str, &str, &str)> = skills
            .iter()
            .zip(&rules)
            .map(|((n, j), r)| (n.as_str(), j.as_str(), r.as_str()))
            .collect();
        let long = "x".repeat(400);
        let index: String = (0..3)
            .map(|i| {
                format!(
                    "- [deploy0 {i} {long}](m{i}{}.md) - deploy0 {long}\n",
                    &long[..240]
                )
            })
            .collect();
        turn_part(dir.path(), "home", "personal", &spec, &index);
        let sent = turn(&ctx(dir.path()), "s", "deploy0 deploy1 deploy2", None);
        let total: usize = sent.iter().map(|i| i.text.len()).sum();
        assert!(total <= TURN_BYTES, "{total} bytes");
        let fits = ids(&sent).iter().filter(|i| i.starts_with("fit.")).count();
        assert_eq!(fits, 3, "the essentials first: {:?}", ids(&sent));
        let memories = ids(&sent)
            .iter()
            .filter(|i| i.starts_with("recall."))
            .count();
        assert!(
            (1..3).contains(&memories),
            "the budget cut some candidates, not all: {:?}",
            ids(&sent)
        );
        assert!(ids(&sent).contains(&FRAMING), "{:?}", ids(&sent));
    }

    #[test]
    fn a_reset_starts_the_count_again_and_only_append_prompts_count() {
        let dir = TempDir::new().unwrap();
        world(dir.path());
        let c = ctx(dir.path());
        turn(&c, "s", "deploy the release", None);
        for _ in 2..=9 {
            turn(&c, "s", "deploy the release", None);
        }
        let asked = Call {
            prompt: Some("deploy the release".into()),
            ..call(Some("s"), Carrier::OnDemand, false)
        };
        for _ in 0..5 {
            expression(&c, &asked).unwrap();
        }
        assert!(
            turn(&c, "s", "deploy the release", None).is_empty(),
            "prompt 10"
        );
        let reset = Call {
            prompt: Some("deploy the release".into()),
            ..call(Some("s"), Carrier::Append, true)
        };
        let after = expression(&c, &reset).unwrap();
        assert!(ids(&after).contains(&"fit.home.deployer"), "{after:?}");
        assert!(
            !ids(&after).contains(&SHORT),
            "the reset cleared the count, so nothing has drifted"
        );
        for n in 2..=10 {
            assert!(
                turn(&c, "s", "deploy the release", None).is_empty(),
                "prompt {n} after reset"
            );
        }
        assert_eq!(ids(&turn(&c, "s", "deploy the release", None)), [SHORT]);
    }

    #[test]
    fn macro_run_5_the_habit_is_a_candidate_and_switched_off_it_is_not() {
        let dir = TempDir::new().unwrap();
        turn_part(
            dir.path(),
            "home",
            "personal",
            &[],
            "- [No awk in scripts](no-awk-in-scripts.md) - shell habit\n",
        );
        let prompt = "Fix data.sh so it prints only the second field of each line";
        let items = turn(&ctx(dir.path()), "s", prompt, None);
        assert_eq!(ids(&items)[0], FRAMING);
        assert!(
            items[1].text.contains("No awk in scripts - shell habit"),
            "{items:?}"
        );
        let mut off = ctx(dir.path());
        off.config.candidates = false;
        assert!(turn(&off, "t", prompt, None).is_empty());
    }

    #[test]
    fn the_framing_line_goes_once_above_the_candidates_and_not_again() {
        let dir = TempDir::new().unwrap();
        world(dir.path());
        let c = ctx(dir.path());
        let first = turn(&c, "s", "deploy the release", None);
        let at = ids(&first).iter().position(|i| *i == FRAMING).unwrap();
        assert_eq!(ids(&first).iter().filter(|i| **i == FRAMING).count(), 1);
        assert!(first[at + 1].id.starts_with("recall."), "{first:?}");
        assert!(
            turn(&c, "s", "deploy the release", None).is_empty(),
            "nothing changed"
        );
    }

    #[test]
    fn a_framing_line_with_no_candidate_under_it_is_dropped() {
        let item = |id: &str| Item {
            id: id.into(),
            text: String::new(),
            data: serde_json::Value::Null,
        };
        assert_eq!(ids(&framed(vec![item("fit.a"), item(FRAMING)])), ["fit.a"]);
        let kept = framed(vec![item(FRAMING), item("recall.x")]);
        assert_eq!(ids(&kept), [FRAMING, "recall.x"]);
    }
}
