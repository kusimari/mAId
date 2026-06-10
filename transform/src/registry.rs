//! Static registry mapping $HOME-facing paths to mAId source paths.
//! `deploy.rs` walks this list and manages symlinks.
//!
//! `home_path` is expanded against the caller's chosen `home`
//! argument (not via env-var lookup) so tests can run against a
//! fake $HOME.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // EntryKind is part of the registry shape; unused today
                    // but documents whether each entry is file or dir.
pub enum EntryKind {
    File,
    Dir,
}

#[derive(Debug, Clone, Copy)]
pub struct Entry {
    /// Path relative to $HOME. e.g. ".claude/CLAUDE.md".
    pub home_subpath: &'static str,
    /// Path relative to the mAId checkout. e.g. "sources/claude/CLAUDE.md".
    pub source_subpath: &'static str,
    #[allow(dead_code)]
    pub kind: EntryKind,
}

pub const REGISTRY: &[Entry] = &[
    Entry {
        home_subpath: ".claude/CLAUDE.md",
        source_subpath: "sources/claude/CLAUDE.md",
        kind: EntryKind::File,
    },
    Entry {
        home_subpath: ".claude/skills",
        source_subpath: "sources/skills",
        kind: EntryKind::Dir,
    },
    Entry {
        home_subpath: ".claude/agents",
        source_subpath: "sources/agents",
        kind: EntryKind::Dir,
    },
    Entry {
        home_subpath: ".claude/commands",
        source_subpath: "sources/commands",
        kind: EntryKind::Dir,
    },
    Entry {
        home_subpath: ".kiro/steering/KIRO.md",
        source_subpath: "sources/kiro/KIRO.md",
        kind: EntryKind::File,
    },
    Entry {
        home_subpath: ".kiro/steering/skills",
        source_subpath: "sources/skills",
        kind: EntryKind::Dir,
    },
];
