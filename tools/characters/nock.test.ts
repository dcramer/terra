import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { nockOf, stringMiddle } from "./nock.ts";
import type { V3 } from "./glb.ts";

const ROOT = join(import.meta.dirname, "..", "..");

test("a string's middle is halfway along it, where it bows out", () => {
  // A string from y -0.5 to 0.7, drawn back to z 0.15 at its middle: its middle is y 0.1.
  const pts: V3[] = [];
  for (let i = 0; i <= 120; i++) {
    const y = -0.5 + i / 100;
    pts.push([0.002, y, 0.15 - 0.1 * Math.abs(y - 0.1)]);
  }
  const m = stringMiddle(pts)!;
  assert.ok(Math.abs(m[1] - 0.1) < 1e-3, `y ${m[1]}`);
  assert.ok(Math.abs(m[2] - 0.15) < 2e-3, `z ${m[2]}`);
  assert.equal(stringMiddle([]), undefined);
});

test("the bow's data gives the nock its look's string has", () => {
  const data = JSON.parse(readFileSync(join(ROOT, "art", "held", "bow.json"), "utf8"));
  const at = nockOf(join(ROOT, "assets", "models", "held", "bow.glb"))!;
  assert.ok(Math.hypot(...at.map((v, i) => v - data.nock[i])) < 0.005, `${at} against ${data.nock}`);
  // A look with no string has no nock to measure.
  assert.equal(nockOf(join(ROOT, "assets", "models", "held", "knife.glb")), undefined);
});
