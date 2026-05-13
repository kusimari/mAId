// Run with `deno task test`, or `deno test -A` for an ad-hoc run.
import { assertEquals, assertThrows } from "@std/assert";
import { parseFile, parseFrontmatter, SchemaError, splitFrontmatter } from "../maid/schema.ts";

Deno.test("splitFrontmatter parses valid file", () => {
  const { raw, body } = splitFrontmatter(
    "foo.md",
    "---\nname: foo\ndescription: bar\n---\nHello body.\n",
  );
  assertEquals(raw, "name: foo\ndescription: bar");
  assertEquals(body, "Hello body.\n");
});

Deno.test("splitFrontmatter rejects missing header", () => {
  assertThrows(
    () => splitFrontmatter("foo.md", "name: foo\n"),
    SchemaError,
    "missing YAML frontmatter",
  );
});

Deno.test("splitFrontmatter rejects unterminated header", () => {
  assertThrows(
    () => splitFrontmatter("foo.md", "---\nname: foo\n"),
    SchemaError,
    "unterminated",
  );
});

Deno.test("parseFrontmatter handles scalars and flow arrays", () => {
  const fm = parseFrontmatter("foo.md", 'name: git\ndescription: "git stuff"\ntags: [a, b, "c d"]');
  assertEquals(fm.name, "git");
  assertEquals(fm.description, "git stuff");
  assertEquals(fm.tags, ["a", "b", "c d"]);
});

Deno.test("parseFrontmatter skips blank + comment lines", () => {
  const fm = parseFrontmatter(
    "foo.md",
    "\n# comment\nname: foo\n\ndescription: bar # trailing comment\n",
  );
  assertEquals(fm.name, "foo");
  assertEquals(fm.description, "bar");
});

Deno.test("parseFrontmatter rejects malformed line", () => {
  assertThrows(
    () => parseFrontmatter("foo.md", "name foo\n"),
    SchemaError,
    "cannot parse frontmatter line",
  );
});

Deno.test("parseFile: valid skill", () => {
  const { frontmatter, body } = parseFile(
    "foo.md",
    "---\nname: foo\ndescription: bar\nversion: 1.0.0\ntags: [a]\n---\nBody.\n",
  );
  assertEquals(frontmatter.name, "foo");
  assertEquals(frontmatter.version, "1.0.0");
  assertEquals(frontmatter.tags, ["a"]);
  assertEquals(body, "Body.\n");
});

Deno.test("parseFile: missing name", () => {
  assertThrows(
    () => parseFile("foo.md", "---\ndescription: bar\n---\n"),
    SchemaError,
    "missing required string field: name",
  );
});

Deno.test("parseFile: missing description", () => {
  assertThrows(
    () => parseFile("foo.md", "---\nname: foo\n---\n"),
    SchemaError,
    "missing required string field: description",
  );
});
