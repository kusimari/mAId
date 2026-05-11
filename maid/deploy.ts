// Create/refresh the $HOME-facing symlinks into the mAId checkout.

import { REGISTRY, type RegistryEntry } from "./registry.ts";

export interface DeployOptions {
  home: string;
  checkout: string;
  dryRun?: boolean;
  force?: boolean;
}

export type DeployResult =
  | { entry: RegistryEntry; status: "created" | "already-ok" | "replaced"; target: string }
  | { entry: RegistryEntry; status: "skipped-missing-source"; target: string }
  | { entry: RegistryEntry; status: "skipped-non-symlink"; target: string; existing: "file" | "dir" }
  | { entry: RegistryEntry; status: "skipped-wrong-symlink"; target: string; currentTarget: string };

export async function deploy(opts: DeployOptions): Promise<DeployResult[]> {
  const results: DeployResult[] = [];
  for (const entry of REGISTRY) {
    results.push(await deployOne(entry, opts));
  }
  return results;
}

async function deployOne(entry: RegistryEntry, opts: DeployOptions): Promise<DeployResult> {
  const homePath = `${opts.home}/${entry.home_subpath}`;
  const sourcePath = `${opts.checkout}/${entry.source_subpath}`;

  // Source must exist for us to point at it.
  if (!(await pathExists(sourcePath))) {
    return { entry, status: "skipped-missing-source", target: homePath };
  }

  // Ensure parent dir exists.
  const parent = homePath.replace(/\/[^/]+$/, "");
  if (!(await pathExists(parent))) {
    if (!opts.dryRun) await Deno.mkdir(parent, { recursive: true });
  }

  // Inspect destination.
  let lstat: Deno.FileInfo | null = null;
  try {
    lstat = await Deno.lstat(homePath);
  } catch {
    lstat = null;
  }

  if (lstat === null) {
    // Fresh create.
    if (!opts.dryRun) await Deno.symlink(sourcePath, homePath);
    return { entry, status: "created", target: homePath };
  }

  if (lstat.isSymlink) {
    const currentTarget = await Deno.readLink(homePath);
    if (currentTarget === sourcePath) {
      return { entry, status: "already-ok", target: homePath };
    }
    if (opts.force) {
      if (!opts.dryRun) {
        await Deno.remove(homePath);
        await Deno.symlink(sourcePath, homePath);
      }
      return { entry, status: "replaced", target: homePath };
    }
    return { entry, status: "skipped-wrong-symlink", target: homePath, currentTarget };
  }

  // Regular file or real directory — never overwrite.
  return {
    entry,
    status: "skipped-non-symlink",
    target: homePath,
    existing: lstat.isDirectory ? "dir" : "file",
  };
}

async function pathExists(p: string): Promise<boolean> {
  try {
    await Deno.lstat(p);
    return true;
  } catch {
    return false;
  }
}
