import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { judge, readBody, readLook } from "../characters/attach_contract.ts";
import { Glb } from "../characters/glb.ts";
import { gripOf, handleOf } from "../characters/grip.ts";
import { boxHead, collar, collarMesh, glbOf, haft, head } from "./fixtures.ts";
import { ROOT } from "./kind.ts";

// The made-up axe written by the item writer (cargo run -p terra_bake --bin item), then read by
// the attachment contract's check exactly as it reads a held look in assets/models/held.
function writeAxe(): string {
  const dir = mkdtempSync(join(tmpdir(), "item-"));
  mkdirSync(join(dir, "models", "parts"), { recursive: true });
  writeFileSync(join(dir, "models", "parts", "test_box_head.glb"), glbOf(boxHead()));
  writeFileSync(join(dir, "models", "parts", "test_collar.glb"), glbOf(collarMesh(), [1, 1, 1], "collar"));
  const out = join(dir, "axe.glb");
  const args = ["run", "-q", "-p", "terra_bake", "--bin", "item", "--", "axe", "test_box_head", "test_haft", "test_collar", "--parts", "-", "--models", dir, "--out", out];
  const run = spawnSync("cargo", args, { cwd: ROOT, input: JSON.stringify([head(), haft(), collar()]), encoding: "utf8" });
  assert.equal(run.status, 0, run.stderr);
  return out;
}

test("an assembled item reads as a held look: its grip at the origin, its haft up +Y, its parts' materials", () => {
  const file = writeAxe();
  const look = readLook(file, "held/test_axe");
  // The fingers' check (grip.ts, shared by moves.ts) finds the haft across the grip, its radius the haft's.
  const handle = handleOf(new Glb(file));
  assert.ok(handle && Math.abs(handle.r - 0.0175) < 1e-3 && Math.hypot(...handle.at) < 1e-3, JSON.stringify(handle));
  assert.equal(gripOf(new Glb(file)), "power");
  const ys = look.points.map((p) => p[1]);
  // The haft from its butt 8 cm below the grip to its top 0.60 m above; the head ends 4 mm under the haft's top.
  assert.ok(Math.abs(Math.min(...ys) + 0.08) < 1e-6 && Math.abs(Math.max(...ys) - 0.6) < 1e-6, `${Math.min(...ys)} to ${Math.max(...ys)}`);
  assert.deepEqual((look.extras.recipe as { kind: string }).kind, "axe");
  // The in-hand rules of a hafted look find its grip on the origin and its length its kind's.
  const faults = judge(look, readBody(), { kind: "hafted" }).filter((f) => ["grip", "along +Y", "size", "kind"].includes(f.rule));
  assert.deepEqual(faults, []);
});

test("the same recipe writes the same file", () => {
  assert.deepEqual(readFileSync(writeAxe()), readFileSync(writeAxe()));
});
