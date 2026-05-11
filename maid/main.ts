// maid — Deno CLI for validating and deploying mAId sources.
//
// Subcommands:
//   validate     Walk sources/, fail on frontmatter errors.
//   deploy       Create/refresh $HOME-facing symlinks into this checkout.
//   status       Print each registered symlink and its resolution state.
//   --help       Usage.

import { walkSources } from "./sources.ts";
import { deploy } from "./deploy.ts";
import { REGISTRY } from "./registry.ts";

const USAGE = `maid — validate/deploy mAId sources.

Usage:
  maid validate              Walk sources/ and validate frontmatter.
  maid deploy [--force]      Create/refresh $HOME-facing symlinks.
  maid status                Report each managed symlink's state.
  maid --help | -h           Show this help.

Flags:
  --dry-run                  (deploy) Plan without making changes.
  --force                    (deploy) Replace symlinks that point elsewhere.
`;

function usage(stream: "out" | "err" = "out") {
  (stream === "out" ? Deno.stdout : Deno.stderr).write(new TextEncoder().encode(USAGE));
}

function repoRoot(): string {
  // main.ts lives at <checkout>/maid/main.ts; walk up one dir.
  const url = new URL("..", import.meta.url);
  return url.pathname.replace(/\/$/, "");
}

function home(): string {
  const h = Deno.env.get("HOME");
  if (!h) {
    console.error("maid: HOME is not set");
    Deno.exit(1);
  }
  return h;
}

async function cmdValidate(): Promise<number> {
  const root = repoRoot();
  try {
    const records = await walkSources(`${root}/sources`);
    console.log(`validated ${records.length} source file(s)`);
    return 0;
  } catch (e) {
    console.error(e instanceof Error ? e.message : String(e));
    return 1;
  }
}

async function cmdDeploy(args: string[]): Promise<number> {
  const dryRun = args.includes("--dry-run");
  const force = args.includes("--force");
  // Validate first so we don't deploy a broken tree.
  const vrc = await cmdValidate();
  if (vrc !== 0) return vrc;

  const results = await deploy({
    home: home(),
    checkout: repoRoot(),
    dryRun,
    force,
  });

  let failures = 0;
  for (const r of results) {
    const tag = dryRun ? "(dry-run) " : "";
    switch (r.status) {
      case "created":
        console.log(`${tag}created   ${r.target}`);
        break;
      case "already-ok":
        console.log(`${tag}ok        ${r.target}`);
        break;
      case "replaced":
        console.log(`${tag}replaced  ${r.target}`);
        break;
      case "skipped-missing-source":
        console.log(`${tag}skip      ${r.target} (source missing)`);
        break;
      case "skipped-non-symlink":
        console.error(
          `${tag}skip      ${r.target} (existing ${r.existing}; not overwriting)`,
        );
        failures++;
        break;
      case "skipped-wrong-symlink":
        console.error(
          `${tag}skip      ${r.target} (points elsewhere: ${r.currentTarget}; use --force to replace)`,
        );
        failures++;
        break;
    }
  }
  return failures > 0 ? 1 : 0;
}

async function cmdStatus(): Promise<number> {
  const h = home();
  const c = repoRoot();
  for (const entry of REGISTRY) {
    const target = `${h}/${entry.home_subpath}`;
    const expected = `${c}/${entry.source_subpath}`;
    let state: string;
    try {
      const lst = await Deno.lstat(target);
      if (lst.isSymlink) {
        const cur = await Deno.readLink(target);
        state = cur === expected ? `ok -> ${cur}` : `WRONG -> ${cur} (expected ${expected})`;
      } else {
        state = `non-symlink (${lst.isDirectory ? "dir" : "file"})`;
      }
    } catch {
      state = "missing";
    }
    console.log(`${entry.home_subpath.padEnd(28)} ${state}`);
  }
  return 0;
}

async function main() {
  const [sub, ...rest] = Deno.args;
  if (!sub || sub === "--help" || sub === "-h") {
    usage();
    Deno.exit(0);
  }
  let rc = 2;
  switch (sub) {
    case "validate":
      rc = await cmdValidate();
      break;
    case "deploy":
      rc = await cmdDeploy(rest);
      break;
    case "status":
      rc = await cmdStatus();
      break;
    default:
      console.error(`maid: unknown subcommand: ${sub}`);
      usage("err");
      rc = 2;
      break;
  }
  Deno.exit(rc);
}

if (import.meta.main) {
  await main();
}
