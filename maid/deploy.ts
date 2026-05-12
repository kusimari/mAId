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
  | {
    entry: RegistryEntry;
    status: "skipped-non-symlink";
    target: string;
    existing: "file" | "dir";
  }
  | {
    entry: RegistryEntry;
    status: "skipped-wrong-symlink";
    target: string;
    currentTarget: string;
  };

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

// ── undeploy ────────────────────────────────────────────────────────

export type UndeployResult =
  | { entry: RegistryEntry; status: "not-deployed"; target: string }
  | { entry: RegistryEntry; status: "removed"; target: string; was: string }
  | {
    entry: RegistryEntry;
    status: "force-removed";
    target: string;
    existing: "file" | "dir" | "symlink";
  }
  | {
    entry: RegistryEntry;
    status: "skipped-foreign-symlink";
    target: string;
    currentTarget: string;
  }
  | {
    entry: RegistryEntry;
    status: "skipped-non-symlink";
    target: string;
    existing: "file" | "dir";
  };

export async function undeploy(opts: DeployOptions): Promise<UndeployResult[]> {
  const results: UndeployResult[] = [];
  for (const entry of REGISTRY) {
    results.push(await undeployOne(entry, opts));
  }
  return results;
}

async function undeployOne(entry: RegistryEntry, opts: DeployOptions): Promise<UndeployResult> {
  const homePath = `${opts.home}/${entry.home_subpath}`;
  const expectedTarget = `${opts.checkout}/${entry.source_subpath}`;

  let lstat: Deno.FileInfo | null = null;
  try {
    lstat = await Deno.lstat(homePath);
  } catch {
    return { entry, status: "not-deployed", target: homePath };
  }

  if (lstat.isSymlink) {
    const currentTarget = await Deno.readLink(homePath);
    if (currentTarget === expectedTarget) {
      if (!opts.dryRun) await Deno.remove(homePath);
      return { entry, status: "removed", target: homePath, was: currentTarget };
    }
    if (opts.force) {
      if (!opts.dryRun) await Deno.remove(homePath);
      return { entry, status: "force-removed", target: homePath, existing: "symlink" };
    }
    return { entry, status: "skipped-foreign-symlink", target: homePath, currentTarget };
  }

  // Regular file / directory at the managed destination.
  const existing: "file" | "dir" = lstat.isDirectory ? "dir" : "file";
  if (opts.force) {
    if (!opts.dryRun) {
      await Deno.remove(homePath, { recursive: lstat.isDirectory });
    }
    return { entry, status: "force-removed", target: homePath, existing };
  }
  return { entry, status: "skipped-non-symlink", target: homePath, existing };
}
