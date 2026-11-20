//! Reinforcement: the short form of the essentials, sent again once they
//! drifted far back or a fit returns (D2). The acting persona and its
//! memory, the work's goal and next step, and each fitting bring-in's name
//! with its first rule lines, under a small byte cap; never a whole
//! `rules` section. See the initiative spec `harness-core`, "Each turn:
//! expression", and project.md, "Expression".

use crate::expression::{clip, one_line, TAG};

/// The whole short form stays under this many bytes.
pub const BYTES: usize = 1_500;
/// A bring-in's first rules, in the short form: this many rule lines...
pub const RULE_LINES: usize = 2;
/// ...each cut to this many bytes.
pub const LINE_BYTES: usize = 240;

/// One essential, as the short form says it.
#[derive(Debug, Clone, PartialEq)]
pub struct Short {
    /// The full item it stands for.
    pub id: String,
    pub text: String,
}

/// The short form of `shorts`, in order: a heading line, then each one
/// whole that still fits under `BYTES`, so one long entry does not keep
/// out the rest. None when there is nothing to say.
pub fn text(shorts: &[Short]) -> Option<String> {
    let mut out = format!("{TAG} still in force, as placed earlier in this session:\n");
    let mut any = false;
    for s in shorts {
        if out.len() + s.text.len() > BYTES {
            continue;
        }
        out.push_str(&s.text);
        any = true;
    }
    any.then_some(out)
}

/// A bring-in's short form: its name, then its first rule lines.
pub fn bring_in(id: String, name: &str, rules: &str) -> Short {
    let mut text = format!("- {}:\n", one_line(name, LINE_BYTES));
    for line in first_rule_lines(rules) {
        text.push_str(&format!("    {line}\n"));
    }
    Short { id, text }
}

/// The first `RULE_LINES` rules of a `rules` section, each one line: a
/// list item with its wrapped lines joined, or a paragraph. Headings,
/// blank lines and fenced code are not rules.
pub fn first_rule_lines(section: &str) -> Vec<String> {
    let mut rules: Vec<String> = vec![];
    let mut open = false;
    let mut fenced = false;
    for line in section.lines() {
        let t = line.trim();
        if t.starts_with("```") || t.starts_with("~~~") {
            fenced = !fenced;
            open = false;
            continue;
        }
        if fenced || t.is_empty() || t.starts_with('#') {
            open = false;
            continue;
        }
        match item(t) {
            Some(rest) => {
                rules.push(rest.to_string());
                open = true;
            }
            None if open => {
                let last = rules.last_mut().expect("an open rule");
                last.push(' ');
                last.push_str(t);
            }
            None => {
                rules.push(t.to_string());
                open = true;
            }
        }
        if rules.len() > RULE_LINES {
            break;
        }
    }
    rules
        .into_iter()
        .take(RULE_LINES)
        .map(|r| clip(&r, LINE_BYTES))
        .collect()
}

/// A list item's text without its marker (`- `, `* `, `+ `, `1. `).
fn item(line: &str) -> Option<&str> {
    for m in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(m) {
            return Some(rest.trim());
        }
    }
    let digits = line.find(|c: char| !c.is_ascii_digit())?;
    (digits > 0)
        .then(|| {
            line[digits..]
                .strip_prefix(". ")
                .or(line[digits..].strip_prefix(") "))
        })
        .flatten()
        .map(str::trim)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_two_rules_are_list_items_with_their_wrapped_lines_joined() {
        let section = "## 0. Stop where you should\n\n- When the user names a checkpoint,\n  stop there.\n- Show the design first.\n- After 3 failed fixes, stop.\n";
        assert_eq!(
            first_rule_lines(section),
            [
                "When the user names a checkpoint, stop there.",
                "Show the design first."
            ]
        );
    }

    #[test]
    fn numbered_items_paragraphs_and_fences() {
        let numbered = "## Rules\n\n1. Tag first.\n2) Then deploy.\n";
        assert_eq!(first_rule_lines(numbered), ["Tag first.", "Then deploy."]);
        let prose = "## Voice\n\nShort sentences,\ncommon words.\n\n```\nnot a rule\n```\n\nNo em dashes.\n";
        assert_eq!(
            first_rule_lines(prose),
            ["Short sentences, common words.", "No em dashes."]
        );
        assert!(first_rule_lines("## Only a heading\n\n").is_empty());
    }

    #[test]
    fn each_rule_line_and_the_whole_short_form_are_capped() {
        let long = format!("- {}\n- b\n", "x".repeat(1_000));
        let lines = first_rule_lines(&long);
        assert!(lines[0].len() <= LINE_BYTES, "{}", lines[0].len());
        let many: Vec<Short> = (0..20)
            .map(|i| bring_in(format!("fit.p.b{i}"), &format!("b{i}"), &long))
            .collect();
        let text = text(&many).unwrap();
        assert!(text.len() <= BYTES, "{}", text.len());
        assert!(text.contains("- b0:\n"));
        assert!(!text.contains("- b19:\n"), "whole entries only, in order");
        let huge = Short {
            id: "reinforce.persona".into(),
            text: "x".repeat(BYTES),
        };
        let text = super::text(&[huge, many[0].clone()]).unwrap();
        assert!(text.contains("- b0:\n"), "an entry too long is passed over");
    }

    #[test]
    fn nothing_to_say_is_no_short_form() {
        assert_eq!(text(&[]), None);
        let one = bring_in(
            "fit.p.deployer".into(),
            "deployer",
            "## Rules\n\nTAG FIRST\n",
        );
        assert_eq!(
            text(&[one]).unwrap(),
            format!("{TAG} still in force, as placed earlier in this session:\n- deployer:\n    TAG FIRST\n")
        );
    }
}
