//! Turn fit: which turn bring-ins apply now, derived from the prompt (the
//! outcome and job), the work's state and the location, never declared.
//! Also the persona guess, which reads the same location signals.

use crate::expression::here::{self, Here};
use ghost::contract::{Declaration, Scope};
use ghost::{BringIn, Ghost};
use std::collections::BTreeSet;
use std::path::Path;

/// At most this many bring-ins fit one turn.
pub const CAP: usize = 3;

#[derive(Debug, Clone, PartialEq)]
pub struct Fit<'a> {
    pub bring_in: &'a BringIn,
    pub score: u32,
    /// What matched, for the item's data.
    pub why: Vec<String>,
}

/// The turn bring-ins that fit, best first, at most `CAP`.
pub fn fit<'a>(ghost: &'a Ghost, prompt: &str, state: Option<&str>, here: &Here) -> Vec<Fit<'a>> {
    let mut all = ranked(ghost, prompt, state, here);
    all.truncate(CAP);
    all
}

/// Every turn bring-in that fits, best first; equal scores keep the
/// ghost's order.
pub fn ranked<'a>(
    ghost: &'a Ghost,
    prompt: &str,
    state: Option<&str>,
    here: &Here,
) -> Vec<Fit<'a>> {
    let said = words(prompt);
    let paths = path_words(prompt);
    let state = state.map(words).unwrap_or_default();
    // A place (a location or a file) weighs 2, a word 1.
    let mut fits: Vec<Fit> = turn(ghost)
        .filter_map(|(b, d)| {
            let mut why: Vec<(u32, String)> = place(d, here).into_iter().map(|w| (2, w)).collect();
            let worded = [("job", &d.signals.jobs), ("outcome", &d.signals.outcomes)]
                .into_iter()
                .flat_map(|(kind, terms)| terms.iter().map(move |t| (kind, t)))
                .filter(|(_, t)| phrase_in(t, &said))
                .map(|(kind, t)| (1, format!("{kind} {t:?}")));
            why.extend(worded);
            why.extend(
                d.signals
                    .states
                    .iter()
                    .filter(|t| phrase_in(t, &state))
                    .map(|t| (1, format!("state {t:?}"))),
            );
            why.extend(
                d.signals
                    .files
                    .iter()
                    .filter(|g| {
                        here::matcher(g).is_ok_and(|m| {
                            paths.iter().any(|p| {
                                m.is_match(p)
                                    || Path::new(p).file_name().is_some_and(|n| m.is_match(n))
                            })
                        })
                    })
                    .map(|g| (2, format!("named file {g:?}"))),
            );
            if no_signals(d) {
                let shared = words(&d.job).intersection(&said).count();
                if shared >= 2 {
                    why.push((1, format!("{shared} words of its job")));
                }
            }
            let score = why.iter().map(|(w, _)| w).sum();
            (score > 0).then(|| Fit {
                bring_in: b,
                score,
                why: why.into_iter().map(|(_, w)| w).collect(),
            })
        })
        .collect();
    fits.sort_by_key(|f| std::cmp::Reverse(f.score));
    fits
}

/// The persona this location points at: the only one there is, or the one
/// whose bring-ins' location or file signals match here, or whose bound
/// state location holds the folder. None when that is not exactly one.
pub fn persona(ghost: &Ghost, here: &Here) -> Option<String> {
    let all: BTreeSet<&str> = ghost
        .personas
        .iter()
        .filter_map(|p| p.persona.as_deref())
        .collect();
    if all.len() == 1 {
        return all.first().map(|p| p.to_string());
    }
    let cwd = here.cwd.as_deref()?;
    let mut pointed: BTreeSet<&str> = turn(ghost)
        .filter(|(_, d)| !place(d, here).is_empty())
        .filter_map(|(b, _)| b.persona.as_deref())
        .collect();
    for b in &ghost.bring_ins {
        let (Some(d), Some(r)) = (b.declaration(), &b.record) else {
            continue;
        };
        let inside = d
            .state
            .iter()
            .flat_map(|s| s.keys())
            .filter_map(|k| r.values.get(k))
            .any(|v| {
                let v = Path::new(v);
                v.is_absolute() && cwd.starts_with(v)
            });
        if inside {
            pointed.extend(b.persona.as_deref());
        }
    }
    match pointed.len() {
        1 => pointed.first().map(|p| p.to_string()),
        _ => None,
    }
}

fn turn(ghost: &Ghost) -> impl Iterator<Item = (&BringIn, &Declaration)> {
    ghost
        .bring_ins
        .iter()
        .filter_map(|b| b.declaration().map(|d| (b, d)))
        .filter(|(_, d)| d.scope == Scope::Turn)
}

/// What of the location matches: a `locations` glob against the folder,
/// its name or its repo root's name, a `files` glob against the files under
/// the root.
fn place(d: &Declaration, here: &Here) -> Vec<String> {
    let mut why = vec![];
    if let Some(cwd) = &here.cwd {
        for l in &d.signals.locations {
            let Ok(m) = here::matcher(l) else { continue };
            let names = [Some(cwd.as_path()), here.root.as_deref()]
                .into_iter()
                .flatten()
                .filter_map(|p| p.file_name());
            if m.is_match(cwd) || names.into_iter().any(|n| m.is_match(n)) {
                why.push(format!("location {l:?}"));
            }
        }
    }
    for g in &d.signals.files {
        if here.glob(g).is_ok_and(|f| !f.is_empty()) {
            why.push(format!("file {g:?}"));
        }
    }
    why
}

fn no_signals(d: &Declaration) -> bool {
    let s = &d.signals;
    [&s.outcomes, &s.jobs, &s.states, &s.locations, &s.files]
        .iter()
        .all(|v| v.is_empty())
}

/// A phrase is in a text when every one of its content words is.
fn phrase_in(phrase: &str, said: &BTreeSet<String>) -> bool {
    let w = words(phrase);
    !w.is_empty() && w.is_subset(said)
}

const STOP: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "can", "do", "for", "from", "i", "if", "in",
    "is", "it", "its", "me", "my", "of", "on", "or", "off", "please", "so", "that", "the", "this",
    "to", "up", "we", "what", "with", "you", "your", "any", "all", "now", "then", "there",
];

/// Lowercase content words, a plural `s` dropped, stopwords left out.
pub fn words(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .filter(|w| !STOP.contains(&w.as_str()))
        .map(
            |w| match w.len() > 3 && w.ends_with('s') && !w.ends_with("ss") {
                true => w[..w.len() - 1].to_string(),
                false => w,
            },
        )
        .collect()
}

/// Words of the prompt that look like a file: they hold a `.` or a `/`.
fn path_words(prompt: &str) -> Vec<String> {
    prompt
        .split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| {
                matches!(
                    c,
                    '`' | '"' | '\'' | ',' | '(' | ')' | ':' | ';' | '?' | '!'
                )
            })
        })
        .map(|w| w.trim_end_matches('.'))
        .filter(|w| w.contains('.') || w.contains('/'))
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::fs;
    use tempfile::TempDir;

    /// A part `name` of `persona` whose bring-ins are skills declared by
    /// `metadata` lines, written under `dir`.
    fn part(dir: &Path, name: &str, persona: &str, skills: &[(&str, &str)]) {
        let parts = dir.join("parts");
        fs::create_dir_all(&parts).unwrap();
        let bring_ins: Vec<Value> = skills
            .iter()
            .map(|(skill, meta)| {
                let p = dir.join(format!("units/{name}/{skill}/SKILL.md"));
                fs::create_dir_all(p.parent().unwrap()).unwrap();
                fs::write(
                    &p,
                    format!("---\nname: {skill}\ndescription: d\nmetadata:\n{meta}---\n"),
                )
                .unwrap();
                json!({"name": skill, "unit": {"standard": "skill", "path": p}})
            })
            .collect();
        let v = json!({"format": 1, "part": name, "harnesses": {}, "persona": persona, "bring_ins": bring_ins});
        fs::write(parts.join(format!("{name}.json")), v.to_string()).unwrap();
    }

    fn turn(job: &str, signals: &str) -> String {
        format!("  my-harness.contract: \"1\"\n  my-harness.scope: turn\n  my-harness.job: {job}\n{signals}")
    }

    fn names(fits: &[Fit]) -> Vec<String> {
        fits.iter().map(|f| f.bring_in.name.clone()).collect()
    }

    #[test]
    fn each_signal_kind_fits_on_its_own() {
        let dir = TempDir::new().unwrap();
        let work = dir.path().join("notes-vault");
        fs::create_dir_all(work.join("specs")).unwrap();
        fs::write(work.join("specs/project.md"), "").unwrap();
        part(
            dir.path(),
            "a",
            "personal",
            &[
                (
                    "loc",
                    &turn("j", "  my-harness.signals.locations: '*-vault'\n"),
                ),
                (
                    "files",
                    &turn("j", "  my-harness.signals.files: specs/project.md\n"),
                ),
                (
                    "jobs",
                    &turn("j", "  my-harness.signals.jobs: draft an email\n"),
                ),
                (
                    "outcomes",
                    &turn("j", "  my-harness.signals.outcomes: shipped release\n"),
                ),
                (
                    "states",
                    &turn("j", "  my-harness.signals.states: review\n"),
                ),
                (
                    "named",
                    &turn("j", "  my-harness.signals.files: '*.xlsx'\n"),
                ),
            ],
        );
        let m = crate::testing::ghost_of(&dir.path().join("parts"));
        let here = Here::at(Some(&work));
        assert_eq!(names(&fit(&m, "", None, &here)), ["loc", "files"]);
        let below = work.join("deep/er");
        fs::create_dir_all(&below).unwrap();
        assert!(
            fit(&m, "", None, &Here::at(Some(&below))).is_empty(),
            "a folder above the location is not the location"
        );
        fs::create_dir_all(work.join(".git")).unwrap();
        assert_eq!(
            names(&fit(&m, "", None, &Here::at(Some(&below)))),
            ["loc", "files"],
            "the repo root's name is"
        );
        let nowhere = Here::default();
        assert_eq!(
            names(&fit(&m, "please draft emails to Ann", None, &nowhere)),
            ["jobs"]
        );
        assert_eq!(
            names(&fit(&m, "until the release is shipped", None, &nowhere)),
            ["outcomes"]
        );
        assert_eq!(
            names(&fit(&m, "go on", Some("review"), &nowhere)),
            ["states"]
        );
        assert_eq!(
            names(&fit(
                &m,
                "sum the totals in `q3/report.xlsx`",
                None,
                &nowhere
            )),
            ["named"]
        );
        let score =
            |prompt: &str, state: Option<&str>, h: &Here| fit(&m, prompt, state, h)[0].score;
        assert_eq!(score("", None, &here), 2, "a location or a file weighs 2");
        assert_eq!(
            score("sum `q3/report.xlsx`", None, &nowhere),
            2,
            "a named file too"
        );
        assert_eq!(score("draft emails", None, &nowhere), 1, "a word weighs 1");
        assert_eq!(score("go on", Some("review"), &nowhere), 1);
        assert!(
            fit(&m, "draft a plan", None, &nowhere).is_empty(),
            "half a phrase is not a fit"
        );
    }

    #[test]
    fn without_signals_two_job_words_fit_and_one_does_not() {
        let dir = TempDir::new().unwrap();
        part(
            dir.path(),
            "a",
            "personal",
            &[("plain", &turn("capture voice transcripts", ""))],
        );
        part(dir.path(), "b", "personal", &[]);
        let m = crate::testing::ghost_of(&dir.path().join("parts"));
        let h = Here::default();
        assert!(fit(&m, "a voice memo", None, &h).is_empty());
        assert_eq!(
            names(&fit(&m, "capture this voice memo", None, &h)),
            ["plain"]
        );
    }

    #[test]
    fn session_and_undeclared_bring_ins_never_fit_and_the_cap_keeps_the_best() {
        let dir = TempDir::new().unwrap();
        let skills: Vec<(String, String)> = (0..5)
            .map(|i| {
                (
                    format!("s{i}"),
                    turn(
                        "j",
                        &format!(
                            "  my-harness.signals.jobs: deploy{}\n",
                            if i == 4 { ", release" } else { "" }
                        ),
                    ),
                )
            })
            .collect();
        let mut all: Vec<(&str, &str)> = skills
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        let session = "  my-harness.contract: \"1\"\n  my-harness.scope: session\n  my-harness.job: deploy things\n  my-harness.signals.jobs: deploy\n";
        all.push(("mem", session));
        all.push(("bare", "  other.key: x\n"));
        part(dir.path(), "a", "personal", &all);
        let m = crate::testing::ghost_of(&dir.path().join("parts"));
        let fits = fit(&m, "deploy the release", None, &Here::default());
        assert_eq!(names(&fits), ["s4", "s0", "s1"]);
        assert_eq!(fits[0].score, 2);
    }

    #[test]
    fn the_persona_is_guessed_from_the_location_only_when_one_is_pointed_at() {
        let dir = TempDir::new().unwrap();
        let (home, job, both) = (
            dir.path().join("home-repo"),
            dir.path().join("job-repo"),
            dir.path().join("both"),
        );
        for (d, f) in [
            (&home, "vault.md"),
            (&job, "service.toml"),
            (&both, "vault.md"),
            (&both, "service.toml"),
        ] {
            fs::create_dir_all(d).unwrap();
            fs::write(d.join(f), "").unwrap();
        }
        part(
            dir.path(),
            "mine",
            "personal",
            &[("n", &turn("j", "  my-harness.signals.files: vault.md\n"))],
        );
        part(
            dir.path(),
            "theirs",
            "work",
            &[(
                "w",
                &turn("j", "  my-harness.signals.files: service.toml\n"),
            )],
        );
        let m = crate::testing::ghost_of(&dir.path().join("parts"));
        assert_eq!(
            persona(&m, &Here::at(Some(&home))).as_deref(),
            Some("personal")
        );
        assert_eq!(persona(&m, &Here::at(Some(&job))).as_deref(), Some("work"));
        assert_eq!(persona(&m, &Here::at(Some(&both))), None);
        assert_eq!(persona(&m, &Here::at(Some(dir.path()))), None);
        assert_eq!(persona(&m, &Here::default()), None);
    }

    #[test]
    fn inside_a_personas_store_is_that_persona() {
        let dir = TempDir::new().unwrap();
        let store = dir.path().join("store");
        fs::create_dir_all(store.join("memory")).unwrap();
        let server = dir.path().join("server.json");
        let decl = json!({"contract": 1, "scope": "session", "job": "memory",
            "config": {"store": {"about": "s"}}, "state": {"permanent": "store"}});
        fs::write(
            &server,
            json!({"name": "m", "_meta": {"my-harness": decl}}).to_string(),
        )
        .unwrap();
        part(dir.path(), "mine", "personal", &[]);
        let parts = dir.path().join("parts");
        let v = json!({"format": 1, "part": "theirs", "harnesses": {}, "persona": "work",
            "bring_ins": [{"name": "m", "unit": {"standard": "mcp", "path": server}, "values": {"store": store}}]});
        fs::write(parts.join("theirs.json"), v.to_string()).unwrap();
        let m = crate::testing::ghost_of(&parts);
        assert_eq!(
            persona(&m, &Here::at(Some(&store.join("memory")))).as_deref(),
            Some("work")
        );
    }

    #[test]
    fn one_persona_is_the_guess_anywhere() {
        let dir = TempDir::new().unwrap();
        part(dir.path(), "a", "personal", &[]);
        part(dir.path(), "b", "personal", &[]);
        let m = crate::testing::ghost_of(&dir.path().join("parts"));
        assert_eq!(persona(&m, &Here::default()).as_deref(), Some("personal"));
    }
}
