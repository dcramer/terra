import assert from "node:assert/strict";
import { test } from "node:test";
import { path } from "./motion.ts";

test("a take's path runs straight ahead at the clip's speed, a point a frame", () => {
  const [constraint] = path(3.2, 6);
  assert.equal(constraint.type, "root2d");
  assert.equal(constraint.frame_indices.length, 180);
  const points = constraint.smooth_root_2d;
  // Straight along +z, sideways not at all.
  assert.ok(points.every(([x]) => x === 0));
  // Six seconds at 3.2 m/s, measured end to end: 19.2 m less the last frame's step.
  const covered = points[points.length - 1][1] - points[0][1];
  assert.ok(Math.abs(covered - 3.2 * (179 / 30)) < 1e-9);
});
