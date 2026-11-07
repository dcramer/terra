// node --test tools/sandbox/front.test.ts: Front's runner says a line a check, and of a failed
// one only what failed (D388). The planted fault (D632): a check that fails must fail the run.
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { run } from "./front.ts";

/// A folder of made-up checks: one that passes noisily, one that fails saying why among noise,
/// one that dies with no verdict.
function checks(): string {
  const dir = mkdtempSync(join(tmpdir(), "terra-front-"));
  const noise = `for (let i = 0; i < 50; i++) console.log("INFO line " + i);`;
  writeFileSync(join(dir, "good.ts"), `${noise}\nconsole.log("✓ good: fine");\n`);
  writeFileSync(join(dir, "bad.ts"), `${noise}\nconsole.error("✗ bad: the host never had its guest");\nprocess.exit(1);\n`);
  writeFileSync(join(dir, "dead.ts"), `${noise}\nprocess.exit(3);\n`);
  return dir;
}

test("checks that pass are a line each and the run passes", () => {
  const dir = checks();
  const { said, failed } = run(["good", "good"], dir, join(dir, "logs"));
  assert.equal(failed, 0);
  assert.equal(said.length, 3, said.join("\n"));
  assert.match(said[0], /^✓ good \(\d+s\)$/);
  assert.match(said[2], /^✓ front/);
});

test("a check that fails fails the run, saying only what failed and where the rest is", () => {
  const dir = checks();
  const { said, failed } = run(["good", "bad", "dead"], dir, join(dir, "logs"));
  assert.equal(failed, 2);
  const all = said.join("\n");
  assert.match(all, /✗ bad \(\d+s\)\n  ✗ bad: the host never had its guest\n  full log: .*front-bad\.log/);
  assert.match(all, /✗ dead \(\d+s\)\n  it ended with no verdict/);
  assert.ok(!all.includes("INFO line"), "a check's noise is not said");
  assert.match(said.at(-1)!, /^✗ front: 2 of its checks failed$/);
  // The noise is kept, in the log named.
  assert.ok(readFileSync(join(dir, "logs", "front-bad.log"), "utf8").includes("INFO line 49"));
});
