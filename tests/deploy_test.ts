import { assert, assertEquals } from "jsr:@std/assert@1";
import { deploy } from "../maid/deploy.ts";
import { REGISTRY } from "../maid/registry.ts";

async function makeCheckout(): Promise<string> {
  const dir = await Deno.makeTempDir({ prefix: "maid-checkout-" });
  await Deno.mkdir(`${dir}/sources/skills`, { recursive: true });
  await Deno.mkdir(`${dir}/sources/agents`, { recursive: true });
  await Deno.mkdir(`${dir}/sources/commands`, { recursive: true });
  await Deno.writeTextFile(`${dir}/CLAUDE.md`, "---\nname: x\ndescription: y\n---\nTop.\n");
  await Deno.writeTextFile(`${dir}/KIRO.md`, "---\nname: x\ndescription: y\n---\nKiro.\n");
  return dir;
}

async function makeHome(): Promise<string> {
  return await Deno.makeTempDir({ prefix: "maid-home-" });
}

Deno.test("deploy: fresh HOME creates every registry entry", async () => {
  const checkout = await makeCheckout();
  const home = await makeHome();
  const results = await deploy({ home, checkout });

  assertEquals(results.length, REGISTRY.length);
  for (const r of results) {
    assertEquals(r.status, "created", `expected created for ${r.entry.home_subpath}`);
    const linkTarget = await Deno.readLink(`${home}/${r.entry.home_subpath}`);
    assertEquals(linkTarget, `${checkout}/${r.entry.source_subpath}`);
  }
});

Deno.test("deploy: second run is a no-op (already-ok)", async () => {
  const checkout = await makeCheckout();
  const home = await makeHome();
  await deploy({ home, checkout });
  const second = await deploy({ home, checkout });
  for (const r of second) {
    assertEquals(r.status, "already-ok", `unexpected status for ${r.entry.home_subpath}: ${r.status}`);
  }
});

Deno.test("deploy: wrong symlink is skipped without --force", async () => {
  const checkout = await makeCheckout();
  const home = await makeHome();
  // Pre-plant a wrong symlink at the first registry target.
  const first = REGISTRY[0];
  const target = `${home}/${first.home_subpath}`;
  await Deno.mkdir(target.replace(/\/[^/]+$/, ""), { recursive: true });
  await Deno.symlink("/nonexistent/elsewhere.md", target);

  const results = await deploy({ home, checkout });
  const firstResult = results.find((r) => r.entry.home_subpath === first.home_subpath);
  assert(firstResult);
  assertEquals(firstResult!.status, "skipped-wrong-symlink");
});

Deno.test("deploy: wrong symlink is replaced with --force", async () => {
  const checkout = await makeCheckout();
  const home = await makeHome();
  const first = REGISTRY[0];
  const target = `${home}/${first.home_subpath}`;
  await Deno.mkdir(target.replace(/\/[^/]+$/, ""), { recursive: true });
  await Deno.symlink("/nonexistent/elsewhere.md", target);

  const results = await deploy({ home, checkout, force: true });
  const firstResult = results.find((r) => r.entry.home_subpath === first.home_subpath);
  assertEquals(firstResult!.status, "replaced");
  const actual = await Deno.readLink(target);
  assertEquals(actual, `${checkout}/${first.source_subpath}`);
});

Deno.test("deploy: dry-run makes no filesystem changes", async () => {
  const checkout = await makeCheckout();
  const home = await makeHome();
  const results = await deploy({ home, checkout, dryRun: true });
  for (const r of results) {
    assertEquals(r.status, "created"); // logical status
    try {
      await Deno.lstat(`${home}/${r.entry.home_subpath}`);
      throw new Error("expected not to find " + r.entry.home_subpath);
    } catch (e) {
      assert(e instanceof Deno.errors.NotFound, "expected NotFound");
    }
  }
});

Deno.test("deploy: pre-existing real file is not overwritten", async () => {
  const checkout = await makeCheckout();
  const home = await makeHome();
  const first = REGISTRY[0];
  const target = `${home}/${first.home_subpath}`;
  await Deno.mkdir(target.replace(/\/[^/]+$/, ""), { recursive: true });
  await Deno.writeTextFile(target, "user content");

  const results = await deploy({ home, checkout });
  const firstResult = results.find((r) => r.entry.home_subpath === first.home_subpath);
  assertEquals(firstResult!.status, "skipped-non-symlink");
  const still = await Deno.readTextFile(target);
  assertEquals(still, "user content");
});
