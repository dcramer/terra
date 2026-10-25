import assert from "node:assert/strict";
import { test } from "node:test";
import { faceOff, gripOff, roundTrip } from "./shield_path.ts";
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

test("a grip in the fist across the palm is on it; one strapped along the forearm is off the fist and square to the palm", () => {
  // A left fist at the origin, its knuckles across +Z (index forward, little finger back), the
  // forearm back along -X.
  const knuckles: [V3, V3] = [[0.02, 0, 0.04], [0.02, 0, -0.04]];
  const inFist = gripOff([0, 0, 0], knuckles, [0, 0, 0], [0, 0, -1]);
  assert.ok(inFist.cm < 1e-9 && inFist.deg < 1e-6, JSON.stringify(inFist));
  // Turned 20 degrees about the forearm's line: 20 off, the bar either way round.
  const r = (20 * Math.PI) / 180;
  for (const s of [1, -1]) {
    const turned = gripOff([0, 0, 0], knuckles, [0, 0, 0], [0, s * Math.sin(r), s * Math.cos(r)]);
    assert.ok(Math.abs(turned.deg - 20) < 1e-6, `${turned.deg}`);
  }
  // Strapped: its bar along the forearm, its grip 7 cm back from the fist and 6.5 cm out of the
  // palm side: sqrt(7^2 + 6.5^2) cm off, square to the palm.
  const strapped = gripOff([0, 0, 0], knuckles, [-0.07, -0.065, 0], [1, 0, 0]);
  assert.ok(Math.abs(strapped.cm - Math.hypot(7, 6.5)) < 1e-6, `${strapped.cm}`);
  assert.ok(Math.abs(strapped.deg - 90) < 1e-6, `${strapped.deg}`);
});
