//! Simplified frontmatter validator. Four checks:
//!
//! 1. File begins with `---\n`.
//! 2. A closing `---` line exists.
//! 3. Between them, a `name:` line is present and non-empty.
//! 4. A `description:` line is present and non-empty.
//!
//! Drops the YAML-flow-array parser path and the `version`/`tags`
//! type checks the TS suite carried — they never caught a real
//! failure. The host AI tools surface anything fancier the moment
//! they fail to load.

use std::fmt;

#[derive(Debug)]
pub struct SchemaError {
    pub file_path: String,
    pub line: usize,
    pub message: String,
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.file_path, self.line, self.message)
    }
}

impl std::error::Error for SchemaError {}

/// Validate frontmatter on `content` from `file_path`. Returns the
/// first error encountered, or `Ok(())` if the four checks pass.
pub fn validate(file_path: &str, content: &str) -> Result<(), SchemaError> {
    // Check 1: header start.
    if !content.starts_with("---\n") && !content.starts_with("---\r\n") {
        return Err(SchemaError {
            file_path: file_path.into(),
            line: 1,
            message: "missing YAML frontmatter (file must start with '---')".into(),
        });
    }

    // Check 2: closing `---`.
    let mut end_line: Option<usize> = None;
    let lines: Vec<&str> = content.split('\n').collect();
    for (i, line) in lines.iter().enumerate().skip(1) {
        if *line == "---" || *line == "---\r" {
            end_line = Some(i);
            break;
        }
    }
    let Some(end) = end_line else {
        return Err(SchemaError {
            file_path: file_path.into(),
            line: 1,
            message: "unterminated YAML frontmatter (no closing '---')".into(),
        });
    };

    // Check 3 + 4: name and description present and non-empty.
    let mut have_name = false;
    let mut have_desc = false;
    for line in &lines[1..end] {
        if let Some(rest) = line.strip_prefix("name:") {
            if !rest.trim().is_empty() {
                have_name = true;
            }
        } else if let Some(rest) = line.strip_prefix("description:") {
            if !rest.trim().is_empty() {
                have_desc = true;
            }
        }
    }
    if !have_name {
        return Err(SchemaError {
            file_path: file_path.into(),
            line: 1,
            message: "missing required field: name".into(),
        });
    }
    if !have_desc {
        return Err(SchemaError {
            file_path: file_path.into(),
            line: 1,
            message: "missing required field: description".into(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_minimal() {
        assert!(validate("foo.md", "---\nname: foo\ndescription: bar\n---\nbody.\n").is_ok());
    }

    #[test]
    fn ok_with_extra_fields() {
        assert!(validate(
            "foo.md",
            "---\nname: foo\ndescription: bar\nversion: 1.0.0\ntags: [a, b]\n---\nbody.\n"
        )
        .is_ok());
    }

    #[test]
    fn missing_header_start() {
        let e = validate("foo.md", "name: foo\n").unwrap_err();
        assert!(e.message.contains("missing YAML frontmatter"));
    }

    #[test]
    fn unterminated_header() {
        let e = validate("foo.md", "---\nname: foo\n").unwrap_err();
        assert!(e.message.contains("unterminated"));
    }

    #[test]
    fn missing_name() {
        let e = validate("foo.md", "---\ndescription: bar\n---\n").unwrap_err();
        assert!(e.message.contains("name"));
    }

    #[test]
    fn missing_description() {
        let e = validate("foo.md", "---\nname: foo\n---\n").unwrap_err();
        assert!(e.message.contains("description"));
    }

    #[test]
    fn empty_name_is_missing() {
        let e = validate("foo.md", "---\nname:\ndescription: bar\n---\n").unwrap_err();
        assert!(e.message.contains("name"));
    }

    #[test]
    fn empty_description_is_missing() {
        let e = validate("foo.md", "---\nname: foo\ndescription:\n---\n").unwrap_err();
        assert!(e.message.contains("description"));
    }

    #[test]
    fn crlf_header_accepted() {
        assert!(validate(
            "foo.md",
            "---\r\nname: foo\r\ndescription: bar\r\n---\r\nbody.\r\n"
        )
        .is_ok());
    }
}
