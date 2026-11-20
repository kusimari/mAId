//! Recall: the few memories that may bear on a turn, under a cap, offered
//! as candidates (`Candidate`). Memories come from every bring-in with a
//! memory index (project.md, Memory); which of them may bear on the turn
//! is a `Selector`'s call, so the cheap lookup here can give way to a
//! local model with no change to its callers. See the initiative spec
//! `harness-core`, "Who does what" (D1).

use crate::expression::fit::words;
use ghost::environment::{contained, index_of};
use ghost::Ghost;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Memory {
    pub persona: Option<String>,
    pub bring_in: String,
    pub title: String,
    pub hook: String,
    pub file: PathBuf,
}

/// What a turn is about, as a selector reads it: the prompt, and the
/// words of the work around it (what the fitting bring-ins are for, what
/// the named files are), which widen the net.
#[derive(Default)]
pub struct Query<'a> {
    pub prompt: &'a str,
    pub about: BTreeSet<String>,
}

pub trait Selector {
    /// At most `cap` of `memories` that bear on `query`, best first.
    fn select(&self, query: &Query, memories: &[Memory], cap: usize) -> Vec<Memory>;
}

/// Each memory's title and hook against the query's words: a word of the
/// prompt counts 2, a word of what the work is about 1. With nothing
/// about, this is the prompt's words alone.
pub struct Lookup;

impl Selector for Lookup {
    fn select(&self, query: &Query, memories: &[Memory], cap: usize) -> Vec<Memory> {
        let said = words(query.prompt);
        let mut scored: Vec<(usize, &Memory)> = memories
            .iter()
            .map(|m| {
                let its = words(&format!("{} {}", m.title, m.hook));
                let score =
                    2 * its.intersection(&said).count() + its.intersection(&query.about).count();
                (score, m)
            })
            .filter(|(n, _)| *n > 0)
            .collect();
        // Stable: ties keep index order.
        scored.sort_by_key(|s| std::cmp::Reverse(s.0));
        scored
            .into_iter()
            .take(cap)
            .map(|(_, m)| m.clone())
            .collect()
    }
}

/// Every memory of every index in the ghost, for `persona` when one is
/// known, and why any index could not be read.
pub fn memories(ghost: &Ghost, persona: Option<&str>) -> (Vec<Memory>, Vec<(String, String)>) {
    let (mut out, mut bad) = (vec![], vec![]);
    for b in &ghost.bring_ins {
        if persona.is_some() && b.persona.as_deref() != persona {
            continue;
        }
        let (store, index) = match index_of(b) {
            None => continue,
            Some(Err(why)) => {
                bad.push((b.name.clone(), why));
                continue;
            }
            Some(Ok(p)) => p,
        };
        let Ok(text) = ghost::contract::read_small(&index) else {
            continue;
        };
        let dir = index.parent().unwrap_or(&store);
        for line in text.lines() {
            let Some((title, file, hook)) = entry(line) else {
                continue;
            };
            let Ok(rel) = dir.strip_prefix(&store).map(|d| d.join(file)) else {
                continue;
            };
            match contained(&store, &rel) {
                Ok(file) => out.push(Memory {
                    persona: b.persona.clone(),
                    bring_in: b.name.clone(),
                    title: title.into(),
                    hook: hook.into(),
                    file,
                }),
                Err(_) => bad.push((b.name.clone(), format!("memory {file} leaves its store"))),
            }
        }
    }
    (out, bad)
}

/// `- [Title](file.md) - hook`.
fn entry(line: &str) -> Option<(&str, &str, &str)> {
    let rest = line.trim().strip_prefix("- [")?;
    let (title, rest) = rest.split_once("](")?;
    let (file, rest) = rest.split_once(')')?;
    let hook = rest.trim().trim_start_matches(['-', '—', ':']).trim();
    Some((title, file, hook))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    struct Fixture {
        dir: TempDir,
    }

    impl Fixture {
        fn new() -> Fixture {
            let dir = TempDir::new().unwrap();
            let decl = json!({"contract": 1, "scope": "session", "job": "memory",
                "config": {"store": {"about": "s"}, "index": {"about": "i"}},
                "state": {"permanent": "store", "promote": "off"}});
            fs::write(
                dir.path().join("server.json"),
                json!({"name": "m", "_meta": {"ghost-in-shell": decl}}).to_string(),
            )
            .unwrap();
            fs::create_dir_all(dir.path().join("parts")).unwrap();
            Fixture { dir }
        }

        /// A part of `persona` with a memory on `store`, its index bound
        /// to `index`, holding `lines`.
        fn part(&self, name: &str, persona: &str, index: &str, lines: &str) -> PathBuf {
            let store = self.dir.path().join(format!("{name}-store"));
            fs::create_dir_all(store.join("memory")).unwrap();
            fs::write(store.join("memory/MEMORY.md"), lines).unwrap();
            let v = json!({"format": 1, "part": name, "harnesses": {}, "persona": persona,
                "bring_ins": [{"name": format!("{name}-memory"),
                    "unit": {"standard": "mcp", "path": self.dir.path().join("server.json")},
                    "values": {"store": store, "index": index}}]});
            fs::write(
                self.dir.path().join(format!("parts/{name}.json")),
                v.to_string(),
            )
            .unwrap();
            store
        }

        fn ghost(&self) -> Ghost {
            crate::testing::ghost_of(&self.dir.path().join("parts"))
        }
    }

    const INDEX: &str = "# Memory\n\n- [Deploy steps](deploy.md) - how a release is deployed\n- [Coffee](coffee.md) - the person likes it black\nnot an entry\n- [Release notes style](notes.md) - short release notes\n";

    fn titles(m: &[Memory]) -> Vec<&str> {
        m.iter().map(|m| m.title.as_str()).collect()
    }

    #[test]
    fn the_index_is_read_from_the_bound_values_and_ranked_by_shared_words() {
        let f = Fixture::new();
        let store = f.part("home", "personal", "memory/MEMORY.md", INDEX);
        let (all, bad) = memories(&f.ghost(), None);
        assert!(bad.is_empty(), "{bad:?}");
        assert_eq!(
            titles(&all),
            ["Deploy steps", "Coffee", "Release notes style"]
        );
        assert_eq!(all[0].file, store.join("memory/deploy.md"));
        assert_eq!(all[0].persona.as_deref(), Some("personal"));
        let q = Query {
            prompt: "write the release notes",
            ..Query::default()
        };
        assert_eq!(
            titles(&Lookup.select(&q, &all, 3)),
            ["Release notes style", "Deploy steps"]
        );
        let tie = Query {
            prompt: "release notes for this deploy",
            ..Query::default()
        };
        assert_eq!(
            titles(&Lookup.select(&tie, &all, 1)),
            ["Deploy steps"],
            "ties keep index order"
        );
        assert!(Lookup
            .select(
                &Query {
                    prompt: "hello",
                    ..Query::default()
                },
                &all,
                3
            )
            .is_empty());
    }

    #[test]
    fn what_the_work_is_about_widens_the_net_below_the_prompts_own_words() {
        let f = Fixture::new();
        f.part("home", "personal", "memory/MEMORY.md", INDEX);
        let (all, _) = memories(&f.ghost(), None);
        let q = Query {
            prompt: "how is the release deployed",
            about: ["coffee".to_string()].into(),
        };
        assert_eq!(
            titles(&Lookup.select(&q, &all, 3)),
            ["Deploy steps", "Release notes style", "Coffee"],
            "a prompt word outweighs a word of the work"
        );
    }

    #[test]
    fn an_index_or_a_memory_that_leaves_the_store_is_not_read() {
        let f = Fixture::new();
        for (name, index) in [
            ("abs", "/etc/passwd"),
            ("up", "../x/MEMORY.md"),
            ("empty", ""),
        ] {
            f.part(name, "personal", index, INDEX);
        }
        let store = f.part("link", "personal", "memory/out.md", "");
        let outside = f.dir.path().join("outside.md");
        fs::write(&outside, INDEX).unwrap();
        std::os::unix::fs::symlink(&outside, store.join("memory/out.md")).unwrap();
        // A folder on the way that leads out, though the index is not there.
        let deep = f.part("deep", "personal", "memory/MEMORY.md", "");
        fs::remove_dir_all(deep.join("memory")).unwrap();
        fs::create_dir_all(f.dir.path().join("elsewhere")).unwrap();
        std::os::unix::fs::symlink(f.dir.path().join("elsewhere"), deep.join("memory")).unwrap();
        f.part(
            "entries",
            "personal",
            "memory/MEMORY.md",
            "- [Away](../../../etc/passwd) - no\n- [Fine](ok.md) - yes\n",
        );
        let (all, bad) = memories(&f.ghost(), None);
        assert_eq!(titles(&all), ["Fine"]);
        let names: Vec<&str> = bad.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "abs-memory",
                "deep-memory",
                "empty-memory",
                "entries-memory",
                "link-memory",
                "up-memory"
            ]
        );
        let g = f.ghost();
        let rows = ::ghost::environment::derive(&g, &Default::default());
        for name in ["abs-memory", "up-memory", "link-memory", "deep-memory"] {
            let row = rows.iter().find(|r| r.name == name).unwrap();
            let why = row.readiness.why().unwrap_or_default();
            assert!(why.contains("leaves its store"), "{name}: {why}");
        }
    }

    #[test]
    fn only_the_acting_personas_memories_when_one_is_known() {
        let f = Fixture::new();
        f.part(
            "home",
            "personal",
            "memory/MEMORY.md",
            "- [Mine](a.md) - mine\n",
        );
        f.part(
            "job",
            "work",
            "memory/MEMORY.md",
            "- [Theirs](b.md) - theirs\n",
        );
        assert_eq!(titles(&memories(&f.ghost(), Some("work")).0), ["Theirs"]);
        assert_eq!(titles(&memories(&f.ghost(), None).0), ["Mine", "Theirs"]);
    }

    #[test]
    fn a_selector_can_be_swapped() {
        struct Last;
        impl Selector for Last {
            fn select(&self, _: &Query, m: &[Memory], cap: usize) -> Vec<Memory> {
                m.iter().rev().take(cap).cloned().collect()
            }
        }
        let f = Fixture::new();
        f.part("home", "personal", "memory/MEMORY.md", INDEX);
        let (all, _) = memories(&f.ghost(), None);
        let picked: &dyn Selector = &Last;
        assert_eq!(
            titles(&picked.select(&Query::default(), &all, 1)),
            ["Release notes style"]
        );
    }
}
