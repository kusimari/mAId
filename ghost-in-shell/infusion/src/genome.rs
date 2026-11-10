//! genome: who the session is, at its start. Every part fused, the
//! persona it acts through, what each bring-in brings and where its state
//! is, the environment as resolved here, and what waits for the person.
//! See the initiative spec `harness-core`, "The five entry points".

use crate::call::{Call, Ctx, Item, Slice};
use crate::expression::delivery::deliver;
use crate::expression::fit;
use crate::expression::here::Here;
use crate::sleep;
use anyhow::{Context, Result};
use ghost::contract::Scope;
use ghost::environment::{self, Readiness};
use ghost::parts::Tenure;
use ghost::{BringIn, Declared, Persona};
use serde::Serialize;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::Path;

/// Who the session is, at its start: the picture `genome` sends.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Picture {
    pub personas: Vec<Persona>,
    /// The persona this session most likely acts through: the only one,
    /// when there is one.
    pub acting: Option<String>,
    pub session: Vec<Line>,
    pub turn: Vec<Line>,
    pub environment: Vec<environment::Row>,
    /// Bring-in names more than one part records.
    pub clashes: Vec<String>,
    /// The shell's records whose shape was not understood, each with why.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub not_understood: Vec<String>,
    pub pending: Pending,
    /// Sessions marked for sleep and not yet reflected on.
    pub waiting: usize,
}

/// One bring-in, as the picture names it.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Line {
    pub part: String,
    pub name: String,
    pub job: Option<String>,
    pub persona: Option<String>,
    pub borrowed_from: Option<String>,
    /// Where its state lives: each state key's bound value, or that the
    /// key is not bound, so a bring-in with memory always says so.
    pub state: Vec<String>,
    /// Its other bound file and dir locations, by key, so a skill never
    /// needs a default path: each config key, not a state tier and not
    /// secret, whose value is an absolute path.
    pub locations: std::collections::BTreeMap<String, String>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Pending {
    pub count: usize,
    pub at: String,
}

pub fn genome(ctx: &Ctx, call: &Call) -> Result<Vec<Item>> {
    let p = picture_at(ctx, &Here::at(call.at.cwd.as_deref()))?;
    let item = Item {
        // One item: any change re-sends the whole picture. A carrier that
        // needs less can split it into one item per section.
        id: "genome".into(),
        text: render_slice(&p, call.slice),
        data: serde_json::to_value(&p)?,
    };
    deliver(ctx, call, vec![item])
}

pub fn picture(ctx: &Ctx) -> Result<Picture> {
    picture_at(ctx, &Here::default())
}

/// The picture as seen from `here`, which the persona guess reads.
pub fn picture_at(ctx: &Ctx, here: &Here) -> Result<Picture> {
    let ghost = ctx.shell.ghost(here.cwd.as_deref())?;
    let acting = fit::persona(&ghost, here);
    let (mut session, mut turn) = (vec![], vec![]);
    for b in &ghost.bring_ins {
        match (&b.declared, b.declaration().map(|d| d.scope)) {
            (Declared::Unreadable { .. }, _) => {}
            (_, Some(Scope::Session)) => session.push(line(b)),
            // Undeclared units are offered by their own description.
            _ => turn.push(line(b)),
        }
    }
    Ok(Picture {
        personas: ghost.personas.clone(),
        acting,
        session,
        turn,
        environment: environment::derive(&ghost, &ctx.host),
        clashes: ghost.clashes.clone(),
        not_understood: ghost.not_understood.clone(),
        pending: pending(ctx)?,
        waiting: sleep::waiting(&ctx.config)?.len(),
    })
}

pub(crate) fn line(b: &BringIn) -> Line {
    let record = b.record.as_ref();
    let values = record.map(|r| &r.values);
    let tiers: Vec<&str> = b
        .declaration()
        .and_then(|d| d.state.as_ref())
        .into_iter()
        .flat_map(|s| s.keys())
        .collect();
    Line {
        part: b.part.clone(),
        name: b.name.clone(),
        job: b.job().map(str::to_string),
        persona: b.persona.clone(),
        borrowed_from: record.and_then(|r| match &r.tenure {
            Tenure::Borrowed { from, .. } => Some(from.clone()),
            Tenure::Own => None,
        }),
        state: tiers
            .iter()
            .map(|k| match values.and_then(|v| v.get(*k)) {
                Some(v) => v.clone(),
                None => format!("{k} not bound"),
            })
            .collect(),
        locations: b
            .declaration()
            .into_iter()
            .flat_map(|d| &d.config)
            .filter(|(k, key)| !key.secret && !tiers.contains(&k.as_str()))
            .filter_map(|(k, _)| {
                let v = values?.get(k)?;
                Path::new(v).is_absolute().then(|| (k.clone(), v.clone()))
            })
            .collect(),
    }
}

/// Decisions waiting for the person: one file each in the pending dir.
fn pending(ctx: &Ctx) -> Result<Pending> {
    let dir = ctx.config.pending();
    let count = match fs::read_dir(&dir) {
        Ok(rd) => rd
            .flatten()
            .filter(|e| {
                let n = e.file_name();
                let n = n.to_string_lossy();
                !n.starts_with('.') && n.ends_with(".json")
            })
            .count(),
        Err(e) if e.kind() == io::ErrorKind::NotFound => 0,
        Err(e) => return Err(e).with_context(|| format!("cannot read {}", dir.display())),
    };
    Ok(Pending {
        count,
        at: dir.display().to_string(),
    })
}

/// The picture in a few lines per section, in a stable order.
pub fn render(p: &Picture) -> String {
    render_slice(p, Slice::Whole)
}

pub fn render_slice(p: &Picture, slice: Slice) -> String {
    let mut s = String::new();
    let personas: Vec<String> = p
        .personas
        .iter()
        .map(|x| match &x.persona {
            Some(name) => format!("{name} ({})", x.part),
            None if !x.declared => format!("no persona ({}, nothing declared)", x.part),
            None => format!("no persona ({})", x.part),
        })
        .collect();
    let _ = writeln!(
        s,
        "personas: {}",
        match personas.is_empty() {
            true => "none (nothing installed says)".to_string(),
            false => personas.join(", "),
        }
    );
    let _ = writeln!(
        s,
        "acting through: {}",
        p.acting
            .as_deref()
            .unwrap_or("not known; ask which persona this work is for")
    );
    for (title, lines) in [
        ("session bring-ins", &p.session),
        ("turn bring-ins", &p.turn),
    ] {
        let _ = writeln!(s, "{title}:{}", if lines.is_empty() { " none" } else { "" });
        for l in lines {
            let mut who = l.persona.clone().unwrap_or_else(|| l.part.clone());
            match &l.borrowed_from {
                Some(from) => who += &format!(", borrowed from {from}"),
                None => who += ", own",
            }
            let _ = write!(
                s,
                "  {}: {} [{who}]",
                l.name,
                l.job.as_deref().unwrap_or("no job given")
            );
            if !l.state.is_empty() {
                let _ = write!(s, "; state: {}", l.state.join(", "));
            }
            if !l.locations.is_empty() {
                let at: Vec<String> = l
                    .locations
                    .iter()
                    .map(|(k, v)| format!("{k} {v}"))
                    .collect();
                let _ = write!(s, "; locations: {}", at.join(", "));
            }
            s.push('\n');
        }
    }
    let ready = p
        .environment
        .iter()
        .filter(|r| r.readiness == Readiness::Ready)
        .count();
    let _ = writeln!(
        s,
        "environment: {ready} of {} bring-ins ready",
        p.environment.len()
    );
    if slice == Slice::SubSession {
        return s;
    }
    let undeclared: Vec<&str> = p
        .environment
        .iter()
        .filter(|r| r.readiness == Readiness::Undeclared)
        .map(|r| r.name.as_str())
        .collect();
    if !undeclared.is_empty() {
        let _ = writeln!(s, "  undeclared: {}", undeclared.join(", "));
    }
    for r in &p.environment {
        if let Some(why) = r.readiness.why() {
            let _ = writeln!(
                s,
                "  {} {} ({}): {why}",
                r.readiness.label(),
                r.name,
                r.part
            );
        }
    }
    for n in &p.not_understood {
        let _ = writeln!(s, "  not understood: {n}");
    }
    if !p.clashes.is_empty() {
        let _ = writeln!(
            s,
            "  clashes: {} ({})",
            p.clashes.len(),
            p.clashes.join(", ")
        );
    }
    let _ = writeln!(
        s,
        "pending: {} decisions ({})",
        p.pending.count, p.pending.at
    );
    if p.waiting > 0 {
        let _ = writeln!(
            s,
            "  to reflect on: {} sessions marked for sleep (the reflect skill takes them)",
            p.waiting
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::call::Carrier;
    use crate::testing::{call, ctx};
    use serde_json::{json, Value};
    use tempfile::TempDir;

    fn part(dir: &Path, name: &str, persona: &str, bring_ins: Value) {
        let parts = dir.join("parts");
        fs::create_dir_all(&parts).unwrap();
        let v = json!({"format": 1, "part": name, "harnesses": {}, "persona": persona, "bring_ins": bring_ins});
        fs::write(parts.join(format!("{name}.json")), v.to_string()).unwrap();
    }

    fn memory(dir: &Path) -> String {
        let p = dir.join("mem.json");
        let decl = json!({"contract": 1, "scope": "session", "job": "memory for the persona",
            "config": {"store": {"about": "the store"}}, "state": {"local": "store"}});
        fs::write(
            &p,
            json!({"name": "mem", "_meta": {"my-harness": decl}}).to_string(),
        )
        .unwrap();
        p.display().to_string()
    }

    #[test]
    fn append_sends_the_picture_once_and_again_on_change_or_reset() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        part(dir.path(), "a", "personal", json!([{"name": "x"}]));
        let append = call(Some("s"), Carrier::Append, false);
        assert_eq!(genome(&c, &append).unwrap().len(), 1);
        assert!(genome(&c, &append).unwrap().is_empty());
        part(
            dir.path(),
            "a",
            "personal",
            json!([{"name": "x"}, {"name": "y"}]),
        );
        assert_eq!(genome(&c, &append).unwrap().len(), 1, "a change re-sends");
        assert!(genome(&c, &append).unwrap().is_empty());
        let reset = call(Some("s"), Carrier::Append, true);
        assert_eq!(genome(&c, &reset).unwrap().len(), 1, "a reset re-sends");
        let sub = call(Some("s.sub-1"), Carrier::Append, false);
        assert_eq!(
            genome(&c, &sub).unwrap().len(),
            1,
            "a sub-session has its own record"
        );
    }
    #[test]
    fn on_demand_always_answers_and_counts_as_sent() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        part(dir.path(), "a", "personal", json!([]));
        let asked = call(Some("s"), Carrier::OnDemand, false);
        assert_eq!(genome(&c, &asked).unwrap().len(), 1);
        assert_eq!(genome(&c, &asked).unwrap().len(), 1);
        assert!(genome(&c, &call(Some("s"), Carrier::Append, false))
            .unwrap()
            .is_empty());
    }
    #[test]
    fn transient_and_sessionless_calls_always_get_everything_and_record_nothing() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        part(dir.path(), "a", "personal", json!([]));
        for _ in 0..2 {
            assert_eq!(
                genome(&c, &call(Some("s"), Carrier::Transient, false))
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(
                genome(&c, &call(None, Carrier::Append, false))
                    .unwrap()
                    .len(),
                1
            );
        }
        assert_eq!(
            genome(&c, &call(Some("s"), Carrier::Append, false))
                .unwrap()
                .len(),
            1
        );
    }
    #[test]
    fn the_genome_names_a_bring_ins_bound_files_but_no_secret_command_or_relative_value() {
        let dir = TempDir::new().unwrap();
        let skill = dir.path().join("style/SKILL.md");
        fs::create_dir_all(skill.parent().unwrap()).unwrap();
        fs::write(&skill, "---\nname: style\ndescription: d\nmetadata:\n  my-harness.contract: \"1\"\n  my-harness.scope: turn\n  my-harness.job: write\n  my-harness.config.learned: \"[optional] older\"\n  my-harness.config.promoted: \"[optional] older\"\n  my-harness.config.token: \"[secret] a token\"\n  my-harness.config.serve: a command\n  my-harness.config.index: relative\n---\n").unwrap();
        part(
            dir.path(),
            "home",
            "personal",
            json!([{"name": "style", "unit": {"standard": "skill", "path": skill},
                "values": {"learned": "/c/style/learned.md", "promoted": "/c/style/promoted.md",
                    "token": "env:TOKEN", "serve": "[\"/bin/x\"]", "index": "memory/MEMORY.md"}}]),
        );
        let p = picture(&ctx(dir.path())).unwrap();
        assert_eq!(
            serde_json::to_value(&p.turn[0].locations).unwrap(),
            json!({"learned": "/c/style/learned.md", "promoted": "/c/style/promoted.md"})
        );
        let text = render(&p);
        assert!(
            text.contains("  style: write [personal, own]; locations: learned /c/style/learned.md, promoted /c/style/promoted.md\n"),
            "{text}"
        );
        let mem = memory(dir.path());
        part(
            dir.path(),
            "home",
            "personal",
            json!([{"name": "mem", "unit": {"standard": "mcp", "path": mem}, "values": {"store": "/s/personal"}}]),
        );
        let text = render(&picture(&ctx(dir.path())).unwrap());
        assert!(
            text.contains("; state: /s/personal\n") && !text.contains("locations"),
            "a state tier is named once, as state:\n{text}"
        );
    }
    #[test]
    fn the_picture_sorts_bring_ins_by_scope_and_names_their_state() {
        let dir = TempDir::new().unwrap();
        let mem = memory(dir.path());
        part(
            dir.path(),
            "home",
            "personal",
            json!([
                {"name": "mem", "unit": {"standard": "mcp", "path": mem}, "values": {"store": "/s/personal"}},
                {"name": "docs", "tenure": {"kind": "borrowed", "from": "a vendor"}}
            ]),
        );
        part(
            dir.path(),
            "job",
            "work",
            json!([{"name": "gone", "unit": {"standard": "skill", "path": "/nowhere"}}]),
        );
        part(
            dir.path(),
            "lab",
            "work",
            json!([{"name": "unbound-mem", "unit": {"standard": "mcp", "path": mem}}]),
        );
        let p = picture(&ctx(dir.path())).unwrap();
        assert_eq!(p.acting, None, "two personas: not guessed");
        assert_eq!(p.session.len(), 2);
        assert_eq!(p.session[0].state, ["/s/personal"]);
        assert_eq!(p.session[1].state, ["store not bound"]);
        assert_eq!(p.turn.len(), 1);
        assert_eq!(p.turn[0].borrowed_from.as_deref(), Some("a vendor"));
        let text = render(&p);
        for want in [
            "personas: personal (home), work (job), work (lab)",
            "acting through: not known",
            "  mem: memory for the persona [personal, own]; state: /s/personal",
            "  docs: no job given [personal, borrowed from a vendor]",
            "environment: 0 of 4 bring-ins ready",
            "  undeclared: docs",
            "  not ready mem (home): store at /s/personal not there",
            "  unreadable gone (job): cannot read /nowhere",
            "pending: 0 decisions",
        ] {
            assert!(text.contains(want), "{want:?} in:\n{text}");
        }
    }
    #[test]
    fn a_sub_session_slice_keeps_who_and_what_and_drops_what_waits_for_the_person() {
        let dir = TempDir::new().unwrap();
        part(dir.path(), "a", "personal", json!([{"name": "docs"}]));
        part(dir.path(), "b", "work", json!([{"name": "docs"}]));
        let p = picture(&ctx(dir.path())).unwrap();
        let whole = render(&p);
        let sub = render_slice(&p, Slice::SubSession);
        for kept in [
            "personas: ",
            "acting through: ",
            "turn bring-ins:",
            "environment: 0 of 2",
        ] {
            assert!(sub.contains(kept), "{kept:?} in:\n{sub}");
        }
        for dropped in ["undeclared:", "clashes:", "pending:"] {
            assert!(whole.contains(dropped), "{dropped:?} in:\n{whole}");
            assert!(!sub.contains(dropped), "{dropped:?} not in:\n{sub}");
        }
    }

    #[test]
    fn clashes_get_a_line_only_when_there_are_any() {
        let dir = TempDir::new().unwrap();
        part(dir.path(), "a", "personal", json!([{"name": "mem"}]));
        let quiet = render(&picture(&ctx(dir.path())).unwrap());
        assert!(!quiet.contains("clashes"), "{quiet}");
        part(dir.path(), "b", "work", json!([{"name": "mem"}]));
        let text = render(&picture(&ctx(dir.path())).unwrap());
        assert!(text.contains("  clashes: 1 (mem)\n"), "{text}");
    }

    #[test]
    fn a_unit_of_an_unknown_contract_is_named_and_an_undeclared_one_has_no_persona() {
        use ghost::installed::{Kind, Unit};
        let dir = TempDir::new().unwrap();
        let skill = |name: &str, contract: &str| {
            let d = dir.path().join(name);
            fs::create_dir_all(&d).unwrap();
            let meta = match contract {
                "" => String::new(),
                c => format!("metadata:\n  my-harness.contract: \"{c}\"\n  my-harness.scope: turn\n  my-harness.job: j\n  my-harness.repository: https://example.org/me/next\n"),
            };
            fs::write(
                d.join("SKILL.md"),
                format!("---\nname: {name}\ndescription: about {name}\n{meta}---\n"),
            )
            .unwrap();
            let mut u = Unit::new(Kind::Skill, name);
            u.manifest = Some(d.join("SKILL.md"));
            u
        };
        let mut c = ctx(dir.path());
        c.shell = Box::new(vec![skill("next-skill", "2"), skill("plain-skill", "")]);
        let text = render(&picture(&c).unwrap());
        assert!(text.starts_with("personas: no persona (local)\n"), "{text}");
        assert!(
            text.contains("  unreadable next-skill (local): the my-harness declaration follows contract 2, which this ghost does not know\n"),
            "{text}"
        );
        assert!(
            text.contains("  plain-skill: about plain-skill [local, borrowed from local]"),
            "{text}"
        );
        assert!(text.contains("  undeclared: plain-skill\n"), "{text}");
    }

    #[test]
    fn a_shell_record_not_understood_is_named_in_the_environment() {
        use ghost::installed::{Installed, Unit};
        struct Changed;
        impl Installed for Changed {
            fn units(&self, _: Option<&Path>) -> anyhow::Result<Vec<Unit>> {
                Ok(vec![])
            }
            fn not_understood(&self, _: Option<&Path>) -> Vec<String> {
                vec![
                    "/c/plugins/installed_plugins.json: shape not understood (Claude Code 9.9.9)"
                        .into(),
                ]
            }
        }
        let dir = TempDir::new().unwrap();
        let mut c = ctx(dir.path());
        c.shell = Box::new(Changed);
        let text = render(&picture(&c).unwrap());
        assert!(
            text.contains("environment: 0 of 0 bring-ins ready\n  not understood: /c/plugins/installed_plugins.json: shape not understood (Claude Code 9.9.9)\n"),
            "{text}"
        );
    }

    #[test]
    fn two_parts_with_one_persona_still_act_through_it() {
        let dir = TempDir::new().unwrap();
        part(dir.path(), "a", "personal", json!([]));
        part(dir.path(), "b", "personal", json!([]));
        assert_eq!(
            picture(&ctx(dir.path())).unwrap().acting.as_deref(),
            Some("personal")
        );
    }

    #[test]
    fn a_bad_session_id_is_refused_whatever_the_carrier() {
        let dir = TempDir::new().unwrap();
        let c = ctx(dir.path());
        for carrier in [Carrier::Append, Carrier::Transient, Carrier::OnDemand] {
            assert!(
                genome(&c, &call(Some("../x"), carrier, false)).is_err(),
                "{carrier:?}"
            );
        }
    }

    #[test]
    fn one_persona_is_the_acting_one_and_pending_counts_its_dir() {
        let dir = TempDir::new().unwrap();
        part(dir.path(), "a", "personal", json!([]));
        let pending = dir.path().join("state/pending");
        fs::create_dir_all(&pending).unwrap();
        for f in ["1.json", "2.json", ".tmp.json", "note.txt"] {
            fs::write(pending.join(f), "{}").unwrap();
        }
        let p = picture(&ctx(dir.path())).unwrap();
        assert_eq!(p.acting.as_deref(), Some("personal"));
        assert_eq!(p.pending.count, 2);
    }
}
