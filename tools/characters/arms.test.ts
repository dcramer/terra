import assert from "node:assert/strict";
import { test } from "node:test";
import { SLICES, reach, turnOut } from "./arms.ts";
import { type V3, rot, sub } from "./glb.ts";

// terra_walker::arms's own case, so the twins agree: a hand hanging straight down from a
// shoulder 0.2 m out, 0.6 m below it, by a hip joint 0.1 m out, is turned out until it is
// 0.2 m out, `OUT` hip widths.
test("a hand hanging against the thigh is turned out until clear, and a clear one is left", () => {
  const middle: V3 = [0, 1, 0];
  const hip: V3 = [0.1, 0.94, 0];
  const shoulder: V3 = [0.15, 1.45, 0];
  const hanging = (x: number): V3 => [x, 0.9, 0.05];
  const angle = turnOut(1, shoulder, [hanging(0.15)], hip, middle);
  const swung = rot([0, 0, Math.sin(angle / 2), Math.cos(angle / 2)], sub(hanging(0.15), shoulder))[0] + shoulder[0];
  assert.ok(angle > 0);
  assert.ok(swung >= 0.195, `${swung} after ${angle}`);
  const mirror = (v: V3): V3 => [-v[0], v[1], v[2]];
  assert.ok(Math.abs(turnOut(-1, mirror(shoulder), [hanging(-0.15)], mirror(hip), middle) + angle) < 1e-9);
  assert.equal(turnOut(1, shoulder, [hanging(0.25)], hip, middle), 0);
  assert.equal(turnOut(1, shoulder, [[0.12, 1.3, 0.1]], hip, middle), 0);
  assert.equal(turnOut(1, shoulder, [[0.12, 0.9, 0.4]], hip, middle), 0);
});

// terra_walker::arms's own case for a look's reach: a haft 4 cm thick and 8 long, one slice to a
// centimetre, keeps each slice's four extremes and none of its middles.
test("a look's reach is each slice's extremes", () => {
  const points: V3[] = [];
  for (let y = 0; y < 8; y++) for (const [x, z] of [[-0.02, 0], [0.02, 0], [0, -0.01], [0, 0.03], [0, 0]]) points.push([x, y * 0.01, z]);
  const got = reach(points);
  assert.equal(got.length, 4 * SLICES);
  assert.ok(!got.some((p) => p[0] === 0 && p[2] === 0), "a middle point kept");
  assert.ok(got.some((p) => p[0] === 0 && Math.abs(p[1] - 0.07) < 1e-9 && p[2] === 0.03));
  assert.deepEqual(reach([]), []);
});
