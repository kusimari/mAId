//! The concept's old name, `my-harness`, read for one release after the
//! rename to ghost-in-shell: the old variables when the new ones are
//! unset, the old state dir where it is when the new one is not there,
//! and declarations under the old namespace. Each use is said in a line,
//! for `ghost status` and the genome. Backlog `drop-my-harness-compat`
//! removes this module and its callers.

use std::path::{Path, PathBuf};

/// The old namespace and state dir.
pub const NAME: &str = "my-harness";
/// The old name of `config::STATE`.
pub const STATE: &str = "MY_HARNESS_STATE";
/// The old name of `config::CANDIDATES`.
pub const CANDIDATES: &str = "MY_HARNESS_CANDIDATES";
/// Plain's old part lists, which stay in the old dir (plain/env.just).
const PARTS: &str = "parts";

/// Which name of a variable to read: `new`, unless only `old` is set.
/// Reading `old` is said in `said`.
pub fn var(
    var: &impl Fn(&str) -> Option<String>,
    new: &'static str,
    old: &'static str,
    said: &mut Vec<String>,
) -> &'static str {
    let set = |k: &str| var(k).is_some_and(|v| !v.is_empty());
    if set(new) || !set(old) {
        return new;
    }
    said.push(format!(
        "{old} is the old name of {new}; read for this release only"
    ));
    old
}

/// The state dir under `base`: the new one, unless only the old one holds
/// the ghost's state, which is then used where it is and said in `said`.
/// An old dir holding only plain's old part lists is no ghost's.
pub fn state_dir(base: &Path, said: &mut Vec<String>) -> PathBuf {
    let (new, old) = (base.join(crate::NAME), base.join(NAME));
    let ghosts = std::fs::read_dir(&old)
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.file_name() != PARTS);
    if new.exists() || !ghosts {
        return new;
    }
    said.push(format!(
        "the state is at {}, the old name; read where it is for this release only, so move all of it but {PARTS}/ to {} once no older ghost is rolled back to",
        old.display(),
        new.display()
    ));
    old
}

/// A bring-in whose declaration is under the old namespace, in a line.
pub fn declared(name: &str, part: &str) -> String {
    format!(
        "{name} ({part}) declares under {NAME}, the old name of {}; read for this release only",
        crate::NAME
    )
}
