// Walk mAId/sources/<kind>/ and return parsed records.

import { parseFile, type Frontmatter, SchemaError } from "./schema.ts";

export type Kind = "skills" | "agents" | "commands";
export const ALL_KINDS: Kind[] = ["skills", "agents", "commands"];

export interface SourceRecord {
  kind: Kind;
  name: string;
  path: string; // absolute or repo-relative; callers pass an absolute root and get back absolute paths
  frontmatter: Frontmatter;
  body: string;
}

/**
 * Walk sources under `rootDir` (e.g. `<mAId>/sources`).
 * Skills: <root>/skills/<name>/SKILL.md
 * Agents/commands: <root>/<kind>/<name>.md
 * Returns records sorted deterministically by (kind, name).
 */
export async function walkSources(rootDir: string): Promise<SourceRecord[]> {
  const records: SourceRecord[] = [];
  const errors: SchemaError[] = [];

  for (const kind of ALL_KINDS) {
    const kindDir = `${rootDir}/${kind}`;
    if (!(await pathExists(kindDir))) continue;

    if (kind === "skills") {
      for await (const entry of Deno.readDir(kindDir)) {
        if (!entry.isDirectory || entry.name.startsWith(".")) continue;
        const skillPath = `${kindDir}/${entry.name}/SKILL.md`;
        if (!(await pathExists(skillPath))) continue;
        try {
          const content = await Deno.readTextFile(skillPath);
          const { frontmatter, body } = parseFile(skillPath, content);
          records.push({ kind, name: entry.name, path: skillPath, frontmatter, body });
        } catch (e) {
          if (e instanceof SchemaError) errors.push(e);
          else throw e;
        }
      }
    } else {
      for await (const entry of Deno.readDir(kindDir)) {
        if (!entry.isFile || !entry.name.endsWith(".md") || entry.name.startsWith(".")) continue;
        const filePath = `${kindDir}/${entry.name}`;
        const name = entry.name.replace(/\.md$/, "");
        try {
          const content = await Deno.readTextFile(filePath);
          const { frontmatter, body } = parseFile(filePath, content);
          records.push({ kind, name, path: filePath, frontmatter, body });
        } catch (e) {
          if (e instanceof SchemaError) errors.push(e);
          else throw e;
        }
      }
    }
  }

  if (errors.length > 0) {
    const msg = errors.map((e) => e.message).join("\n");
    throw new Error(`Schema validation failed:\n${msg}`);
  }

  records.sort((a, b) => {
    if (a.kind !== b.kind) return a.kind < b.kind ? -1 : 1;
    return a.name < b.name ? -1 : a.name > b.name ? 1 : 0;
  });
  return records;
}

async function pathExists(p: string): Promise<boolean> {
  try {
    await Deno.lstat(p);
    return true;
  } catch {
    return false;
  }
}
