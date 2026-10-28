import assert from "node:assert/strict";
import { test } from "node:test";
import { type Q, qmul, type V3 } from "./glb.ts";
import { bendOf, type HandAxes, WRIST } from "./wrist.ts";
import { limited } from "./wrist_limit.ts";

// A forearm along +X, the hand straight on it at rest: its line +X, palm -Y, thumb +Z.
const axes: HandAxes = { along: [1, 0, 0], palm: [0, -1, 0], thumb: [0, 0, 1] };
const [elbow, wrist]: V3[] = [
  [0, 0, 0],
  [0.25, 0, 0],
];
const rest = { flex: 0, dev: 0 };
const about = (axis: V3, deg: number): Q => {
  const s = Math.sin((deg * Math.PI) / 360);
  return [axis[0] * s, axis[1] * s, axis[2] * s, Math.cos((deg * Math.PI) / 360)];
};
// The hand bent toward its thumb (radial) and toward its palm (flexion) by these degrees.
const radial = (deg: number) => about([0, -1, 0], deg);
const flexed = (deg: number) => about([0, 0, -1], deg);

test("a wrist within a person's range is left as it is", () => {
  const r = radial(12);
  const out = limited(elbow, wrist, r, axes, rest);
  assert.ok(out.turned < 1e-3);
  assert.deepEqual(out.r, r);
});

test("a wrist past radial is turned back to the limit, by what it was over", () => {
  assert.ok(Math.abs(bendOf(elbow, wrist, radial(25), axes).dev - 25) < 1e-9, "the case is 25° radial");
  const out = limited(elbow, wrist, radial(25), axes, rest);
  assert.ok(Math.abs(bendOf(elbow, wrist, out.r, axes).dev - WRIST.radial) < 0.01);
  assert.ok(Math.abs(out.turned - 5) < 0.01);
});

test("extension and ulnar deviation at once both end at their limits, measured from the rest", () => {
  const off = { flex: 4, dev: -3 };
  const out = limited(elbow, wrist, qmul(flexed(-(WRIST.ext + 9) + off.flex), radial(-(WRIST.ulnar + 6) + off.dev)), axes, off);
  const b = bendOf(elbow, wrist, out.r, axes);
  assert.ok(b.flex - off.flex >= -WRIST.ext - 0.05, `extension ${-(b.flex - off.flex)}`);
  assert.ok(b.dev - off.dev >= -WRIST.ulnar - 0.05, `ulnar ${-(b.dev - off.dev)}`);
  assert.ok(out.turned > 9 && out.turned < 9 + 6 + 1, `turned ${out.turned}`);
});
