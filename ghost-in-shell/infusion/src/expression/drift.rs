//! Drift: how far back the essentials were last placed in a session, in
//! prompts and in bytes, and so when their short form is due again (D2,
//! "Reinforce on change, and again once drifted"). Code decides it from the
//! delivery record alone. See the initiative spec `harness-core`, "Who does
//! what" (D2), and project.md, "Expression".

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The short form is due once the essentials were last placed this many
/// prompts back.
pub const TURNS: u32 = 10;
/// Or this many bytes back: of the session's conversation where the
/// carrier names its file (what was said and done is what pushes rules out
/// of reach; the shell's own bookkeeping in that file is not), else of the
/// prompts themselves.
pub const BYTES: u64 = 200_000;
/// The state file in the session's delivery record.
pub const FILE: &str = "drift.json";

/// A point in a session: its prompt count and how many bytes it had.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Mark {
    pub turn: u32,
    pub at: u64,
}

/// What a session's delivery record keeps for drift.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, Eq)]
pub struct Drift {
    /// This prompt.
    pub now: Mark,
    /// The fits of the prompt before, by item id.
    pub fits: BTreeSet<String>,
    /// Where the essentials were last placed whole, in full or short.
    pub placed: Option<Mark>,
}

impl Drift {
    /// The state after `self` for the next prompt: one turn on, at `size`
    /// bytes when the carrier knows the conversation's size, else `prompt`
    /// bytes further on.
    pub fn next(&self, size: Option<u64>, prompt: u64) -> Drift {
        Drift {
            now: Mark {
                turn: self.now.turn.saturating_add(1),
                at: size.unwrap_or(self.now.at.saturating_add(prompt)),
            },
            ..self.clone()
        }
    }

    /// Whether the essentials last placed are far enough back to place
    /// again: `TURNS` prompts or `BYTES` bytes. Never placed is not: the
    /// first essentials of a session go in full, or are the genome's.
    pub fn drifted(&self) -> bool {
        match self.placed {
            None => false,
            Some(p) => {
                self.now.turn.saturating_sub(p.turn) >= TURNS
                    || self.now.at.saturating_sub(p.at) >= BYTES
            }
        }
    }

    /// The fits of this prompt that did not fit the one before: a fit
    /// coming back. On a session's first prompt nothing comes back.
    pub fn returning(&self, fits: &BTreeSet<String>) -> BTreeSet<String> {
        match self.now.turn {
            0 | 1 => BTreeSet::new(),
            _ => fits.difference(&self.fits).cloned().collect(),
        }
    }

    /// The state to keep after this prompt: its fits, and the mark moved
    /// here when the essentials were placed whole.
    pub fn after(self, fits: BTreeSet<String>, placed_whole: bool) -> Drift {
        Drift {
            fits,
            placed: match placed_whole {
                true => Some(self.now),
                false => self.placed,
            },
            ..self
        }
    }

    pub fn read(text: Option<&str>) -> Drift {
        // A state file not understood reads as a fresh session: the worst
        // case is one short form sent early.
        text.and_then(|t| serde_json::from_str(t).ok())
            .unwrap_or_default()
    }

    pub fn write(&self) -> String {
        serde_json::to_string(self).expect("drift is plain data")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(turn: u32, at: u64) -> Mark {
        Mark { turn, at }
    }

    fn placed(p: Mark, now: Mark) -> Drift {
        Drift {
            now,
            placed: Some(p),
            ..Drift::default()
        }
    }

    #[test]
    fn due_at_ten_prompts_and_not_at_nine() {
        assert!(!placed(at(1, 0), at(10, 0)).drifted());
        assert!(placed(at(1, 0), at(11, 0)).drifted());
    }

    #[test]
    fn due_at_two_hundred_kb_and_not_just_under() {
        assert!(!placed(at(1, 1_000), at(2, 1_000 + BYTES - 1)).drifted());
        assert!(placed(at(1, 1_000), at(2, 1_000 + BYTES)).drifted());
    }

    #[test]
    fn never_placed_is_not_drifted() {
        assert!(!Drift::default().next(None, BYTES).drifted());
    }

    #[test]
    fn bytes_come_from_the_conversation_when_known_else_from_the_prompts() {
        let d = Drift::default().next(None, 40).next(None, 60);
        assert_eq!(d.now, at(2, 100));
        let d = d.next(Some(9_000), 60);
        assert_eq!(d.now, at(3, 9_000), "the conversation's size, not a sum");
    }

    #[test]
    fn the_mark_moves_only_when_the_essentials_were_placed_whole() {
        let d = Drift::default().next(None, 1);
        let d = d.after(BTreeSet::new(), true);
        assert_eq!(d.placed, Some(at(1, 1)));
        let d = d.next(None, 1).after(BTreeSet::new(), false);
        assert_eq!(d.placed, Some(at(1, 1)), "not placed whole: kept");
    }

    #[test]
    fn a_fit_that_returns_is_one_the_last_prompt_lacked() {
        let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<BTreeSet<_>>();
        let first = Drift::default().next(None, 1);
        assert!(first.returning(&ids(&["a"])).is_empty(), "first prompt");
        let d = first.after(ids(&["a"]), true).next(None, 1);
        assert!(d.returning(&ids(&["a"])).is_empty());
        let both = ids(&["a", "b"]);
        let back: Vec<String> = d.returning(&both).into_iter().collect();
        assert_eq!(back, ["b"]);
    }

    #[test]
    fn a_state_file_not_understood_reads_as_fresh() {
        assert_eq!(Drift::read(Some("not json")), Drift::default());
        let d = Drift::default().next(None, 3).after(BTreeSet::new(), true);
        assert_eq!(Drift::read(Some(&d.write())), d);
    }
}
