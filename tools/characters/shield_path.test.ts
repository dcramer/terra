import assert from "node:assert/strict";
import { test } from "node:test";
import { faceOff, roundTrip } from "./shield_path.ts";
import type { V3 } from "./glb.ts";

const still = (p: V3) => () => p;

test("a shield held where it was through the move change makes no round trip", () => {
  assert.ok(roundTrip(still([0.3, 1.1, 0.2]), still([0.3, 1.1, 0.2])).m < 1e-9);
});

test("an incoming clip that starts with the shield in at the chest and eases it back out makes a round trip", () => {
  // Out at the side (0.4 m left) in the outgoing clip; the incoming starts it at the chest
  // (0 m) and eases it back out by 0.25 s. Crossfaded over 0.11 s, the shown shield is
  // farthest in near the blend's end: at 0.11 s the incoming has it 0.4 * (1 - 0.11 / 0.25)
  // in, shown at full weight.
  const into = (u: number): V3 => [0.4 * Math.min(1, u / 0.25), 1, 0];
  const r = roundTrip(still([0.4, 1, 0]), into);
  assert.ok(Math.abs(r.m - 0.4 * (1 - 0.11 / 0.25)) < 0.01, `round trip ${r.m}`);
  assert.ok(Math.abs(r.at - 0.11) < 0.01, `at ${r.at}`);
});

test("a shield moved one way by the incoming clip counts as far as it went within the span", () => {
  // The incoming clip holds the shield 0.2 m lower throughout: once blended in, 0.2 m.
  const r = roundTrip(still([0, 1, 0]), still([0, 0.8, 0]));
  assert.ok(Math.abs(r.m - 0.2) < 1e-9);
});

test("a board's face is measured from the chest's forward laid flat", () => {
  assert.equal(faceOff([0, 0, 1], [0, 0, 1]), 0);
  assert.ok(Math.abs(faceOff([0, 0, -1], [0, 0, 1]) - 180) < 1e-9);
  assert.ok(Math.abs(faceOff([1, 0, 1], [0, 0, 1]) - 45) < 1e-9);
  // Face up, a chest bowed forward: 90, the bow taken out of the forward.
  assert.ok(Math.abs(faceOff([0, 1, 0], [0, -0.5, 0.8]) - 90) < 1e-9);
});
