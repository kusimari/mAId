//! Claude's user settings file, read and written so every other key and
//! its order is kept. Shared by each step that owns one setting there.

use crate::record::Item;
use anyhow::{bail, Context, Result};
use std::fs;
use std::io::{self, Write};
use std::path::Path;

pub const NO_CLI: &str = "claude not on PATH";

/// The outcome of one pass over the setting: the line to print, and the
/// part-list entry (none once mAId holds nothing there).
pub struct Outcome {
    pub line: String,
    pub item: Option<Item>,
}

pub fn read_settings(at: &Path) -> Result<serde_json::Map<String, serde_json::Value>> {
    match fs::read_to_string(at) {
        Ok(text) if text.trim().is_empty() => Ok(Default::default()),
        Ok(text) => match serde_json::from_str(&text)
            .with_context(|| format!("{} is not JSON", at.display()))?
        {
            serde_json::Value::Object(map) => Ok(map),
            _ => bail!("{} is not a JSON object", at.display()),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Default::default()),
        Err(e) => Err(e).with_context(|| format!("cannot read {}", at.display())),
    }
}

/// Values are re-serialised: key order is kept, but a number or escape may
/// come back in another text form.
pub fn write_settings(at: &Path, map: &serde_json::Map<String, serde_json::Value>) -> Result<()> {
    let text = serde_json::to_string_pretty(map)?;
    write_atomic(at, format!("{text}\n").as_bytes())
}

/// Write through a temp file and rename, so a reader never sees half a
/// file. A link is written through to the file it names, and that file
/// keeps its mode (a new one gets 0644), so a file another tool manages
/// stays where and what it was.
pub fn write_atomic(at: &Path, bytes: &[u8]) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let target = match fs::symlink_metadata(at) {
        Ok(m) if m.file_type().is_symlink() => {
            fs::canonicalize(at).with_context(|| format!("{} links to nothing", at.display()))?
        }
        _ => at.to_path_buf(),
    };
    let mode = fs::metadata(&target)
        .map(|m| m.permissions())
        .unwrap_or_else(|_| fs::Permissions::from_mode(0o644));
    let dir = target.parent().context("a file needs a parent dir")?;
    fs::create_dir_all(dir)?;
    let mut tmp = tempfile::NamedTempFile::new_in(dir)
        .with_context(|| format!("cannot write beside {}", target.display()))?;
    tmp.write_all(bytes)?;
    fs::set_permissions(tmp.path(), mode)?;
    tmp.persist(&target)?;
    Ok(())
}
