// Minimal YAML frontmatter parser + schema validator.
//
// SKILL.md / agent.md / command.md files carry YAML frontmatter
// between two `---` delimiters. We don't need a full YAML library
// for the shape we use: top-level scalars and (rarely) flow-style
// string arrays. Everything else is an error.

export interface Frontmatter {
  name: string;
  description: string;
  version?: string;
  tags?: string[];
  [key: string]: string | string[] | undefined;
}

export interface ParsedFile {
  frontmatter: Frontmatter;
  body: string;
}

export class SchemaError extends Error {
  constructor(public readonly filePath: string, public readonly line: number, message: string) {
    super(`${filePath}:${line}: ${message}`);
    this.name = "SchemaError";
  }
}

/** Split a file into frontmatter + body. Throws on malformed boundaries. */
export function splitFrontmatter(filePath: string, content: string): { raw: string; body: string } {
  if (!content.startsWith("---\n") && !content.startsWith("---\r\n")) {
    throw new SchemaError(filePath, 1, "missing YAML frontmatter (file must start with '---')");
  }
  const lines = content.split("\n");
  // find the closing '---'
  let end = -1;
  for (let i = 1; i < lines.length; i++) {
    if (lines[i] === "---" || lines[i] === "---\r") {
      end = i;
      break;
    }
  }
  if (end === -1) {
    throw new SchemaError(filePath, 1, "unterminated YAML frontmatter (no closing '---')");
  }
  const raw = lines.slice(1, end).join("\n");
  const body = lines.slice(end + 1).join("\n");
  return { raw, body };
}

/** Parse a minimal YAML frontmatter string. Supports: scalars, flow-style string arrays, comments. */
export function parseFrontmatter(filePath: string, raw: string): Frontmatter {
  const out: Frontmatter = { name: "", description: "" };
  const lines = raw.split("\n");
  for (let i = 0; i < lines.length; i++) {
    const rawLine = lines[i];
    const line = rawLine.replace(/\s+#.*$/, "").trimEnd(); // strip trailing comments
    if (line.trim() === "" || line.trim().startsWith("#")) continue;

    const m = line.match(/^([A-Za-z][A-Za-z0-9_-]*)\s*:\s*(.*)$/);
    if (!m) {
      throw new SchemaError(filePath, i + 2, `cannot parse frontmatter line: ${rawLine}`);
    }
    const [, key, valRaw] = m;
    const val = valRaw.trim();

    if (val.startsWith("[") && val.endsWith("]")) {
      const inner = val.slice(1, -1).trim();
      out[key] = inner === "" ? [] : inner.split(",").map((s) => unquote(s.trim()));
    } else {
      out[key] = unquote(val);
    }
  }
  return out;
}

function unquote(s: string): string {
  if (s.length >= 2) {
    if (s[0] === '"' && s[s.length - 1] === '"') return s.slice(1, -1);
    if (s[0] === "'" && s[s.length - 1] === "'") return s.slice(1, -1);
  }
  return s;
}

/** Validate a parsed frontmatter. Returns the file path + error list (empty on success). */
export function validate(filePath: string, fm: Frontmatter): SchemaError[] {
  const errs: SchemaError[] = [];
  if (!fm.name || typeof fm.name !== "string") {
    errs.push(new SchemaError(filePath, 1, "missing required string field: name"));
  }
  if (!fm.description || typeof fm.description !== "string") {
    errs.push(new SchemaError(filePath, 1, "missing required string field: description"));
  }
  if (fm.version !== undefined && typeof fm.version !== "string") {
    errs.push(new SchemaError(filePath, 1, "version must be a string"));
  }
  if (fm.tags !== undefined && !Array.isArray(fm.tags)) {
    errs.push(new SchemaError(filePath, 1, "tags must be a flow-style array"));
  }
  return errs;
}

/** Parse + validate a SKILL.md / agent.md / command.md file. */
export function parseFile(filePath: string, content: string): ParsedFile {
  const { raw, body } = splitFrontmatter(filePath, content);
  const fm = parseFrontmatter(filePath, raw);
  const errs = validate(filePath, fm);
  if (errs.length > 0) throw errs[0];
  return { frontmatter: fm, body };
}
