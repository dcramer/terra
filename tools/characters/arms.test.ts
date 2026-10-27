import assert from "node:assert/strict";
import { test } from "node:test";
import { turnOut } from "./arms.ts";
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
