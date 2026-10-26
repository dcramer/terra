import assert from "node:assert/strict";
import { test } from "node:test";
import { constraints, path, uthanaArgs, wanted } from "./motion.ts";

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
  // Its heading left to the take.
  assert.equal(constraint.global_root_heading, undefined);
});

test("a strafe's path runs to the body's right at its speed, facing ahead throughout", () => {
  const [constraint] = path(1.2, 6, 90);
  const [x, z] = constraint.smooth_root_2d[179];
  // Facing Kimodo's +z, the body's right is -x; ahead it goes not at all.
  assert.ok(Math.abs(x + 1.2 * (179 / 30)) < 1e-9);
  assert.equal(z, 0);
  assert.equal(constraint.global_root_heading?.length, 180);
  assert.ok(constraint.global_root_heading?.every(([c, s]) => c === 1 && s === 0));
  // A strafe to the left goes the other way.
  assert.ok(Math.abs(path(1.2, 6, -90)[0].smooth_root_2d[179][0] - 1.2 * (179 / 30)) < 1e-9);
});

test("a one-shot is held to nothing, a strafe to its path", () => {
  assert.deepEqual(constraints({ prompt: "", speed: 0, seconds: 3, seed: 1, once: true }), []);
  assert.equal(constraints({ prompt: "", speed: 1.2, seconds: 6, seed: 1, way: 90 })[0].frame_indices.length, 180);
});

test("a paid take already made is never made again: only the missing ones are wanted", () => {
  const made = new Set([0, 2]);
  assert.deepEqual(wanted(3, (n) => made.has(n)), [1]);
  assert.deepEqual(wanted(4, (n) => made.has(n)), [1, 3]);
  assert.deepEqual(wanted(2, () => true), []);
});

test("an Uthana take is made by 3.0 unless its entry names a model; 2.0 takes a seed a take, and its foot IK", () => {
  assert.deepEqual(uthanaArgs({ seed: 1, seconds: 8 }, 2), { model: "3", length: 8, footIk: false });
  assert.deepEqual(uthanaArgs({ model: "2", seed: 1, seconds: 8, foot_ik: true }, 0), { model: "2", length: 8, footIk: true, seed: 1 });
  assert.equal(uthanaArgs({ model: "2", seed: 1, seconds: 8 }, 3).seed, 4);
  // 3.0 makes 4 to 10 whole seconds; 2.0 is not held to it.
  assert.throws(() => uthanaArgs({ seed: 1, seconds: 12 }, 0));
  assert.equal(uthanaArgs({ model: "2", seed: 1, seconds: 12 }, 0).length, 12);
});
