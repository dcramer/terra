import { test } from "node:test";
import assert from "node:assert/strict";
import { FOREARM_DOWN, OBLIQUE, about, atWrist, elbowOpen, inFist } from "./carry_hand.ts";
import { type V3, len, rot } from "./glb.ts";

const rise = (v: V3) => (Math.asin(v[1] / len(v)) * 180) / Math.PI;
const angle = (a: V3, b: V3) => (Math.acos((a[0] * b[0] + a[1] * b[1] + a[2] * b[2]) / (len(a) * len(b))) * 180) / Math.PI;

// The twin of terra_walker::carry_hand's test: a jog's forearm swung up ahead is lowered until it
// points FOREARM_DOWN below level, its upper arm left; a hanging one is left; one that cannot get
// there opens only so far.
test("a carried tool keeps its forearm down and its elbow bent", () => {
  const upper: V3 = [0, -0.28, 0.08];
  const fore: V3 = [0, 0, 0.26];
  const open = elbowOpen(upper, fore);
  assert.ok(open, "a level forearm is lowered");
  const lowered = rot(about(open.axis, -open.open), fore);
  assert.ok(Math.abs(rise(lowered) + FOREARM_DOWN) < 1, `${rise(lowered)}`);
  assert.ok(Math.abs(len(lowered) - len(fore)) < 1e-4);
  assert.ok(angle(lowered, upper) < angle(fore, upper), "the elbow less bent");
  assert.ok(lowered[2] > 0 && Math.abs(lowered[0]) < 1e-4, "ahead still, in the arm's plane");
  assert.equal(elbowOpen(upper, [0, -0.25, 0.05]), undefined, "a hanging forearm is left");
  const raised: V3 = [0, 0, 0.28];
  const bent: V3 = [0, 0.2, 0.15];
  const most = elbowOpen(raised, bent);
  assert.ok(most);
  assert.ok(Math.abs(angle(rot(about(most.axis, -most.open), bent), raised) - 10) < 1, "stops short of straight");
});

// The fist takes the tilt up to OBLIQUE, the wrist the rest, so the two always add to it.
test("a carried tilt is shared between the fist and the wrist", () => {
  for (const tilt of [-45, -32, -OBLIQUE, -5, 0, 20]) {
    assert.ok(Math.abs(inFist(tilt)) <= OBLIQUE);
    assert.equal(inFist(tilt) + atWrist(tilt), tilt);
  }
  assert.equal(atWrist(-5), 0);
});
