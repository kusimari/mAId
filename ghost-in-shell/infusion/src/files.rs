//! Writing the core's own files: whole or not at all.

use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Write `bytes` to a temp file beside `at`, then rename it over `at`, so a
/// reader never sees half a file. A link at `at` is replaced, not followed.
/// The file keeps the mode it had; a new one is `0600`.
pub fn write_atomic(at: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = at
        .parent()
        .ok_or_else(|| io::Error::other("a file needs a parent dir"))?;
    fs::create_dir_all(dir)?;
    let name = at
        .file_name()
        .ok_or_else(|| io::Error::other("a file needs a name"))?
        .to_string_lossy();
    let tmp = dir.join(format!(".{name}.{}.tmp", std::process::id()));
    // A file at the temp name (a planted link, a crash's leftover) is
    // removed, never written through.
    let _ = fs::remove_file(&tmp);
    // A new file is the person's alone: these hold what sessions said.
    let mode = fs::metadata(at).map_or(0o600, |m| m.permissions().mode());
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    fs::rename(&tmp, at).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_file_whole_and_leaves_no_temp() {
        let dir = tempfile::TempDir::new().unwrap();
        let at = dir.path().join("sub/x.json");
        write_atomic(&at, b"one").unwrap();
        write_atomic(&at, b"two").unwrap();
        assert_eq!(fs::read_to_string(&at).unwrap(), "two");
        let names: Vec<_> = fs::read_dir(dir.path().join("sub"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["x.json"]);
        assert_eq!(
            fs::metadata(&at).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
