// Static registry mapping $HOME-facing paths to mAId source paths.
// `deploy.ts` walks this list and manages symlinks.
//
// home_path is expanded via the passed `home` argument (not $HOME
// lookup) so tests can run against a fake HOME.

export type EntryKind = "file" | "dir";

export interface RegistryEntry {
  home_subpath: string; // relative to $HOME, e.g. ".claude/CLAUDE.md"
  source_subpath: string; // relative to the mAId checkout, e.g. "CLAUDE.md"
  kind: EntryKind;
}

export const REGISTRY: RegistryEntry[] = [
  { home_subpath: ".claude/CLAUDE.md", source_subpath: "CLAUDE.md", kind: "file" },
  { home_subpath: ".claude/skills", source_subpath: "sources/skills", kind: "dir" },
  { home_subpath: ".claude/agents", source_subpath: "sources/agents", kind: "dir" },
  { home_subpath: ".claude/commands", source_subpath: "sources/commands", kind: "dir" },
  { home_subpath: ".kiro/steering/KIRO.md", source_subpath: "KIRO.md", kind: "file" },
];
