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

test("a shield moved straight one way by the incoming clip makes no round trip", () => {
  // The incoming clip holds the shield 0.2 m lower throughout: it goes straight there.
  assert.ok(roundTrip(still([0, 1, 0]), still([0, 0.8, 0])).m < 1e-9);
  // From the guard (0 m) straight out to the side (0.4 m) for the blow: no way back.
  const out = (u: number): V3 => [0.4 * Math.min(1, u / 0.25), 1, 0];
  assert.ok(roundTrip(still([0, 1, 0]), out).m < 1e-9);
});

test("a shield easing back in when the next blow starts with it out comes straight out again", () => {
  // The outgoing eases the shield in (0.4 m out, in at 1 m/s); the incoming holds it out.
  // Shown, it goes in while the outgoing still weighs, then out: the dip is the farthest in,
  // shown(u) = 0.4 - u (1 - u / 0.11), deepest at u = 0.055 s, 0.0275 m in.
  const easing = (u: number): V3 => [0.4 - u, 1, 0];
  const r = roundTrip(easing, still([0.4, 1, 0]));
  assert.ok(Math.abs(r.m - 0.055 * (1 - 0.5)) < 0.002, `round trip ${r.m}`);
});

test("a board's face is measured from the chest's forward laid flat", () => {
  assert.equal(faceOff([0, 0, 1], [0, 0, 1]), 0);
  assert.ok(Math.abs(faceOff([0, 0, -1], [0, 0, 1]) - 180) < 1e-9);
  assert.ok(Math.abs(faceOff([1, 0, 1], [0, 0, 1]) - 45) < 1e-9);
  // Face up, a chest bowed forward: 90, the bow taken out of the forward.
  assert.ok(Math.abs(faceOff([0, 1, 0], [0, -0.5, 0.8]) - 90) < 1e-9);
});
